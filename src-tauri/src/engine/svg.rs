use super::input::{error, PrepareError};
use flate2::read::GzDecoder;
use rimage::codecs::svg::{
    resvg::usvg::{self, ImageKind},
    SvgDecoder,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

const CONTENT_LIMIT: u64 = 64 * 1024 * 1024;
const PARSING_MULTIPLIER: u64 = 64;

pub(super) struct LoadedSvg {
    pub tree: usvg::Tree,
    pub resources: Vec<(PathBuf, [u8; 32])>,
    pub parsing_bytes: u64,
    pub raster_bytes: u64,
}

struct Resources {
    root: PathBuf,
    budget: u64,
    expanded_bytes: u64,
    raster_bytes: u64,
    fingerprints: Vec<(PathBuf, [u8; 32])>,
    failure: Option<PrepareError>,
    executing: bool,
}

fn invalid(detail: impl ToString) -> PrepareError {
    error(
        "input.svg_resource_invalid",
        "errors.svgResourceInvalid",
        detail,
    )
}

pub(super) fn load(bytes: &[u8], path: &Path, budget: u64) -> Result<LoadedSvg, PrepareError> {
    load_inner(bytes, path, budget, false)
}

pub(super) fn load_for_execution(
    bytes: &[u8],
    path: &Path,
    budget: u64,
) -> Result<LoadedSvg, PrepareError> {
    load_inner(bytes, path, budget, true)
}

fn load_inner(
    bytes: &[u8],
    path: &Path,
    budget: u64,
    executing: bool,
) -> Result<LoadedSvg, PrepareError> {
    let root = path
        .parent()
        .unwrap_or(Path::new("."))
        .canonicalize()
        .map_err(invalid)?;
    let state = Arc::new(Mutex::new(Resources {
        root: root.clone(),
        budget,
        expanded_bytes: 0,
        raster_bytes: 0,
        fingerprints: Vec::new(),
        failure: None,
        executing,
    }));
    let tree = parse(bytes, &root, 0, &state)?;
    let mut resources = state.lock().unwrap();
    if let Some(failure) = resources.failure.take() {
        return Err(failure);
    }
    Ok(LoadedSvg {
        tree,
        resources: std::mem::take(&mut resources.fingerprints),
        parsing_bytes: resources
            .expanded_bytes
            .saturating_mul(PARSING_MULTIPLIER)
            .saturating_add(8 * 1024 * 1024),
        raster_bytes: resources.raster_bytes,
    })
}

fn expand(bytes: &[u8], state: &Arc<Mutex<Resources>>) -> Result<Vec<u8>, PrepareError> {
    let (remaining, available) = {
        let resources = state.lock().unwrap();
        (
            CONTENT_LIMIT.saturating_sub(resources.expanded_bytes),
            (resources.budget.saturating_sub(8 * 1024 * 1024) / PARSING_MULTIPLIER)
                .saturating_sub(resources.expanded_bytes),
        )
    };
    let limit = remaining.min(available);
    let mut data = Vec::new();
    let reader: Box<dyn Read + '_> = if bytes.starts_with(&[0x1f, 0x8b]) {
        Box::new(GzDecoder::new(bytes))
    } else {
        Box::new(Cursor::new(bytes))
    };
    reader
        .take(limit + 1)
        .read_to_end(&mut data)
        .map_err(invalid)?;
    if data.len() as u64 > remaining {
        return Err(invalid("Expanded SVG resources exceed 64 MiB"));
    }
    let mut resources = state.lock().unwrap();
    let required = resources
        .expanded_bytes
        .saturating_add(data.len() as u64)
        .saturating_mul(PARSING_MULTIPLIER)
        .saturating_add(8 * 1024 * 1024);
    if required > resources.budget {
        return Err(PrepareError::NeedsBudget(
            required.max(resources.budget.saturating_mul(2)),
        ));
    }
    resources.expanded_bytes += data.len() as u64;
    Ok(data)
}

fn parse(
    bytes: &[u8],
    directory: &Path,
    depth: usize,
    state: &Arc<Mutex<Resources>>,
) -> Result<usvg::Tree, PrepareError> {
    if depth > 8 {
        return Err(invalid("SVG nesting exceeds 8 levels"));
    }
    let data = expand(bytes, state)?;
    let text = std::str::from_utf8(&data).map_err(invalid)?;
    let document = roxmltree::Document::parse(text).map_err(invalid)?;
    if document.root_element().tag_name().name() != "svg" {
        return Err(invalid("Input is not an SVG document"));
    }
    for node in document.descendants().filter(|node| node.is_element()) {
        if matches!(node.tag_name().name(), "script" | "foreignObject") {
            return Err(invalid("Scripts and foreign objects are forbidden"));
        }
        if node.tag_name().name() == "image"
            && !node
                .attributes()
                .any(|attribute| attribute.name() == "href")
        {
            return Err(invalid("SVG image has no resource reference"));
        }
        for attribute in node.attributes() {
            if attribute.name().starts_with("on") {
                return Err(invalid("SVG event handlers are forbidden"));
            }
            if attribute.name() == "href"
                && node.tag_name().name() != "image"
                && !attribute.value().starts_with('#')
            {
                return Err(invalid(
                    "External SVG references are only allowed on images",
                ));
            }
            if attribute.name() == "href" && node.tag_name().name() == "image" {
                if attribute.value().to_ascii_lowercase().starts_with("data:") {
                    let url = data_url::DataUrl::process(attribute.value()).map_err(invalid)?;
                    url.decode(|_| Ok::<(), std::convert::Infallible>(()))
                        .map_err(|detail| invalid(format!("{detail:?}")))?;
                } else {
                    validate_resource_path(attribute.value(), directory, state)?;
                }
            }
            validate_urls(attribute.value())?;
        }
        if node.tag_name().name() == "style" {
            validate_urls(node.text().unwrap_or_default())?;
        }
    }
    let mut options = SvgDecoder::parser_options();
    let directory = directory.to_path_buf();
    let data_directory = directory.clone();
    let string_state = Arc::clone(state);
    options.image_href_resolver.resolve_string = Box::new(move |href, _| {
        resolve_result(
            (|| {
                let path = validate_resource_path(href, &directory, &string_state)?;
                let root = string_state.lock().unwrap().root.clone();
                let metadata = fs::metadata(&path).map_err(invalid)?;
                if !metadata.is_file() || metadata.len() > CONTENT_LIMIT {
                    return Err(invalid("Invalid SVG resource size or type"));
                }
                let remaining = {
                    let resources = string_state.lock().unwrap();
                    CONTENT_LIMIT.saturating_sub(resources.expanded_bytes)
                };
                if metadata.len() > remaining {
                    return Err(invalid("SVG resources exceed 64 MiB"));
                }
                let required = {
                    let resources = string_state.lock().unwrap();
                    (resources.expanded_bytes + metadata.len())
                        .saturating_mul(PARSING_MULTIPLIER)
                        .saturating_add(8 * 1024 * 1024)
                };
                if required > string_state.lock().unwrap().budget {
                    return Err(PrepareError::NeedsBudget(required));
                }
                let mut data = Vec::new();
                fs::File::open(&path)
                    .map_err(invalid)?
                    .take(metadata.len() + 1)
                    .read_to_end(&mut data)
                    .map_err(invalid)?;
                if data.len() as u64 != metadata.len() {
                    return Err(invalid("SVG resource changed during inspection"));
                }
                string_state
                    .lock()
                    .unwrap()
                    .fingerprints
                    .push((path.clone(), Sha256::digest(&data).into()));
                resource(
                    &data,
                    path.parent().unwrap_or(&root),
                    depth + 1,
                    &string_state,
                )
            })(),
            &string_state,
        )
    });
    let data_state = Arc::clone(state);
    options.image_href_resolver.resolve_data = Box::new(move |_, bytes, _| {
        resolve_result(
            resource(&bytes, &data_directory, depth + 1, &data_state),
            &data_state,
        )
    });
    let tree = usvg::Tree::from_data(&data, &options).map_err(invalid)?;
    if let Some(failure) = state.lock().unwrap().failure.take() {
        return Err(failure);
    }
    Ok(tree)
}

fn validate_resource_path(
    href: &str,
    directory: &Path,
    state: &Arc<Mutex<Resources>>,
) -> Result<PathBuf, PrepareError> {
    if href.is_empty() || href.contains(':') || href.starts_with(['/', '\\']) {
        return Err(invalid(
            "Network, absolute or empty resource paths are forbidden",
        ));
    }
    let path = directory.join(href).canonicalize().map_err(invalid)?;
    if !path.starts_with(&state.lock().unwrap().root) {
        return Err(invalid("SVG resource escapes the source directory tree"));
    }
    Ok(path)
}

fn resource(
    bytes: &[u8],
    directory: &Path,
    depth: usize,
    state: &Arc<Mutex<Resources>>,
) -> Result<ImageKind, PrepareError> {
    let kind = if bytes.starts_with(b"\x89PNG") {
        Some(0)
    } else if bytes.starts_with(b"\xff\xd8") {
        Some(1)
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some(2)
    } else {
        None
    };
    if let Some(kind) = kind {
        let data = expand(bytes, state)?;
        if kind != 1 {
            super::input::reject_chunk_animation(&data, kind == 2)?;
        }
        let size = imagesize::blob_size(&data).map_err(invalid)?;
        let mut resources = state.lock().unwrap();
        resources.raster_bytes = resources.raster_bytes.saturating_add(
            (size.width as u64)
                .saturating_mul(size.height as u64)
                .saturating_mul(16),
        );
        let required = resources
            .expanded_bytes
            .saturating_mul(PARSING_MULTIPLIER)
            .saturating_add(resources.raster_bytes)
            .saturating_add(8 * 1024 * 1024);
        if required > resources.budget {
            return Err(PrepareError::NeedsBudget(required));
        }
        let executing = resources.executing;
        drop(resources);
        if executing {
            use zune_image::traits::DecoderTrait;
            if kind == 2 {
                rimage::codecs::webp::WebPDecoder::try_new(Cursor::new(&data))
                    .map_err(invalid)?
                    .decode()
                    .map_err(invalid)?;
            } else {
                let format = if kind == 0 {
                    zune_image::codecs::ImageFormat::PNG
                } else {
                    zune_image::codecs::ImageFormat::JPEG
                };
                format
                    .decoder_with_options(
                        zune_core::bytestream::ZCursor::new(data.as_slice()),
                        zune_core::options::DecoderOptions::default()
                            .set_max_width(usize::MAX)
                            .set_max_height(usize::MAX),
                    )
                    .map_err(invalid)?
                    .decode()
                    .map_err(invalid)?;
            }
        }
        let data = Arc::new(data);
        Ok(match kind {
            0 => ImageKind::PNG(data),
            1 => ImageKind::JPEG(data),
            _ => ImageKind::WEBP(data),
        })
    } else {
        Ok(ImageKind::SVG(parse(bytes, directory, depth, state)?))
    }
}

fn resolve_result(
    result: Result<ImageKind, PrepareError>,
    state: &Arc<Mutex<Resources>>,
) -> Option<ImageKind> {
    match result {
        Ok(kind) => Some(kind),
        Err(error) => {
            state.lock().unwrap().failure.get_or_insert(error);
            None
        }
    }
}

fn validate_urls(text: &str) -> Result<(), PrepareError> {
    let normalized = text.to_ascii_lowercase();
    if normalized.contains("@import") {
        return Err(invalid("CSS imports are forbidden"));
    }
    for tail in normalized.split("url(").skip(1) {
        let target = tail
            .split(')')
            .next()
            .unwrap_or_default()
            .trim()
            .trim_matches(['\'', '"']);
        if !target.starts_with('#') {
            return Err(invalid("External CSS resources are forbidden"));
        }
    }
    Ok(())
}
