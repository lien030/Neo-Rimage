use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use walkdir::WalkDir;

use crate::{
    domain::{
        AppError, BackupPolicy, CollisionPolicy, CreateJobRequest, DiscoveredInput, ErrorCategory,
        ErrorContext, InputAcceptancePolicy, InputResource, InputResourceKind, ItemId, ItemSpec,
        JobId, JobSpec, OutputLocation, OutputPlan, RejectedInput, ScanInputsCommand,
        ScanInputsResponse, TimestampMs, IPC_SCHEMA_VERSION,
    },
    engine::encoder_output_extension,
    jobs::JobSubmission,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NormalizationOptions {
    pub maximum_files: usize,
    pub maximum_depth: usize,
}

impl Default for NormalizationOptions {
    fn default() -> Self {
        Self {
            maximum_files: 100_000,
            maximum_depth: 64,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalizedJob {
    pub submission: JobSubmission,
    pub rejected_inputs: Vec<RejectedInput>,
}

#[derive(Clone, Copy, Debug)]
pub struct RequestNormalizer {
    options: NormalizationOptions,
}

impl Default for RequestNormalizer {
    fn default() -> Self {
        Self::new(NormalizationOptions::default())
    }
}

impl RequestNormalizer {
    pub fn new(options: NormalizationOptions) -> Self {
        Self { options }
    }

    pub fn scan_inputs(&self, command: ScanInputsCommand) -> Result<ScanInputsResponse, AppError> {
        validate_schema(command.schema_version)?;
        let resources: Vec<_> = command
            .paths
            .iter()
            .map(|path| InputResource {
                kind: if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir()) {
                    InputResourceKind::Directory
                } else {
                    InputResourceKind::File
                },
                path: path.clone(),
                scan_recursively: command.scan_recursively,
            })
            .collect();
        let (candidates, rejected_inputs) = self.discover_inputs(&resources)?;
        let inputs = candidates
            .into_iter()
            .map(|candidate| {
                let path = candidate.path.to_string_lossy().into_owned();
                #[cfg(windows)]
                let path = windows_path_spelling(&path);
                DiscoveredInput {
                    path,
                    file_name: candidate
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                }
            })
            .collect();
        Ok(ScanInputsResponse {
            schema_version: IPC_SCHEMA_VERSION,
            correlation_id: command.correlation_id,
            inputs,
            rejected_inputs,
        })
    }

    pub fn normalize(&self, request: CreateJobRequest) -> Result<NormalizedJob, AppError> {
        validate_request_shape(&request)?;
        let input_acceptance = request.input_acceptance;
        let inputs = request.inputs.clone();
        let (candidates, rejected_inputs) = self.discover_inputs(&inputs)?;

        if input_acceptance == InputAcceptancePolicy::RejectAll && !rejected_inputs.is_empty() {
            return Err(discovery_rejected_error(&rejected_inputs));
        }
        if candidates.is_empty() {
            return Err(AppError::new(
                "input.no_supported_files",
                ErrorCategory::Input,
                "errors.noSupportedInputs",
                "No supported image files were found in the selected inputs.",
            ));
        }

        let job = JobSpec::with_defaults(JobId::default(), TimestampMs(0), request.clone());
        let items = plan_items(&request, candidates)?;
        Ok(NormalizedJob {
            submission: JobSubmission::new(job, items),
            rejected_inputs,
        })
    }

    fn discover_inputs(
        &self,
        inputs: &[InputResource],
    ) -> Result<(Vec<InputCandidate>, Vec<RejectedInput>), AppError> {
        let mut candidates = Vec::new();
        let mut rejected = Vec::new();
        let mut seen = HashSet::new();

        for (input_index, input) in inputs.iter().enumerate() {
            let input_index = u32::try_from(input_index).unwrap_or(u32::MAX);
            match discover_resource(input_index, input, self.options) {
                Ok(discovered) => {
                    let mut accepted_from_resource = 0usize;
                    for candidate in discovered.accepted {
                        let key = path_identity(&candidate.path);
                        if seen.insert(key) {
                            candidates.push(candidate);
                            accepted_from_resource += 1;
                            if candidates.len() > self.options.maximum_files {
                                return Err(AppError::new(
                                    "input.file_limit_exceeded",
                                    ErrorCategory::Input,
                                    "errors.inputFileLimitExceeded",
                                    "The selected inputs contain more files than this job can accept.",
                                )
                                .with_message_arg(
                                    "maximumFiles",
                                    self.options.maximum_files.to_string(),
                                ));
                            }
                        }
                    }
                    rejected.extend(discovered.rejected);
                    if accepted_from_resource == 0
                        && input.kind == InputResourceKind::Directory
                        && !discovered.had_supported_file
                    {
                        rejected.push(rejected_input(
                            input_index,
                            &input.path,
                            input_error(
                                "input.directory_no_supported_files",
                                "errors.directoryHasNoSupportedFiles",
                                "The directory does not contain supported image files.",
                                Path::new(&input.path),
                            ),
                        ));
                    }
                }
                Err(error) => rejected.push(rejected_input(input_index, &input.path, error)),
            }
        }

        Ok((candidates, rejected))
    }
}

#[derive(Debug)]
struct InputCandidate {
    path: PathBuf,
    scan_root: Option<PathBuf>,
}

#[derive(Debug, Default)]
struct DiscoveryResult {
    accepted: Vec<InputCandidate>,
    rejected: Vec<RejectedInput>,
    had_supported_file: bool,
}

/// Tracks every filesystem identity claimed by one job while its output plan
/// is built. Keeping the three namespaces together makes it harder to add a
/// new output/backup path without checking it against the other two.
struct OutputReservations {
    inputs: HashSet<PathIdentity>,
    outputs: HashSet<PathIdentity>,
    backups: HashSet<PathIdentity>,
}

impl OutputReservations {
    fn new(candidates: &[InputCandidate]) -> Self {
        Self {
            inputs: candidates
                .iter()
                .map(|candidate| path_identity(&candidate.path))
                .collect(),
            outputs: HashSet::new(),
            backups: HashSet::new(),
        }
    }

    fn reserve_output(
        &mut self,
        initial: PathBuf,
        collision: CollisionPolicy,
        current_input: &Path,
    ) -> Result<PathBuf, AppError> {
        if collision == CollisionPolicy::AutoRename {
            for index in 0..10_000u32 {
                let candidate = if index == 0 {
                    initial.clone()
                } else {
                    numbered_path(&initial, index)?
                };
                let key = path_identity(&candidate);
                if !candidate.exists()
                    && !self.outputs.contains(&key)
                    && !self.backups.contains(&key)
                    && (!self.inputs.contains(&key) || paths_equal(&candidate, current_input))
                {
                    self.outputs.insert(key);
                    return Ok(candidate);
                }
            }
            return Err(output_error(
                "output.auto_rename_exhausted",
                "errors.outputNameUnavailable",
                "A unique output name could not be reserved.",
                &initial,
                true,
            ));
        }

        let key = path_identity(&initial);
        if self.outputs.contains(&key) || self.backups.contains(&key) {
            return Err(output_error(
                "output.job_collision",
                "errors.outputCollision",
                "Multiple inputs in this job resolve to the same output path.",
                &initial,
                false,
            ));
        }
        if self.inputs.contains(&key) && !paths_equal(&initial, current_input) {
            return Err(output_error(
                "output.overwrites_other_input",
                "errors.outputOverwritesInput",
                "An output path would overwrite another input in the same job.",
                &initial,
                false,
            ));
        }
        if collision == CollisionPolicy::Fail && initial.exists() {
            return Err(output_error(
                "output.already_exists",
                "errors.outputAlreadyExists",
                "The output file already exists.",
                &initial,
                false,
            ));
        }
        self.outputs.insert(key);
        Ok(initial)
    }

    fn reserve_backup(&mut self, path: PathBuf) -> Result<PathBuf, AppError> {
        let key = path_identity(&path);
        if path.exists()
            || self.outputs.contains(&key)
            || self.inputs.contains(&key)
            || !self.backups.insert(key)
        {
            return Err(output_error(
                "output.backup_collision",
                "errors.backupAlreadyExists",
                "The planned backup path already exists or conflicts with another backup.",
                &path,
                false,
            ));
        }
        Ok(path)
    }
}

fn validate_schema(received: u16) -> Result<(), AppError> {
    if received != IPC_SCHEMA_VERSION {
        return Err(AppError::new(
            "protocol.schema_version_unsupported",
            ErrorCategory::Protocol,
            "errors.schemaVersionUnsupported",
            "The request schema version is not supported by this backend.",
        )
        .with_message_arg("received", received.to_string())
        .with_message_arg("supported", IPC_SCHEMA_VERSION.to_string()));
    }
    Ok(())
}

fn validate_request_shape(request: &CreateJobRequest) -> Result<(), AppError> {
    validate_schema(request.schema_version)?;
    if request.inputs.is_empty() {
        return Err(AppError::new(
            "input.empty",
            ErrorCategory::Validation,
            "errors.inputRequired",
            "Select at least one input file or directory.",
        ));
    }
    validate_suffix(&request.output.suffix)?;
    if request.output.existing_output_backup == BackupPolicy::Enabled
        && request.output.collision != CollisionPolicy::Replace
    {
        return Err(AppError::new(
            "output.backup_requires_replace",
            ErrorCategory::Validation,
            "errors.backupRequiresReplace",
            "Existing-output backup requires the replace collision policy.",
        ));
    }
    if let OutputLocation::Directory(path) = &request.output.location {
        if path.trim().is_empty() {
            return Err(AppError::new(
                "output.directory_empty",
                ErrorCategory::Validation,
                "errors.outputDirectoryRequired",
                "Select an output directory.",
            ));
        }
        let path = Path::new(path);
        if path.exists() && !path.is_dir() {
            return Err(output_directory_not_directory(path));
        }
    }
    Ok(())
}

fn validate_suffix(suffix: &str) -> Result<(), AppError> {
    let unsafe_character = suffix.chars().any(|character| {
        character.is_control()
            || matches!(
                character,
                '/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*'
            )
    });
    if unsafe_character || suffix.contains("..") {
        return Err(AppError::new(
            "output.suffix_unsafe",
            ErrorCategory::Validation,
            "errors.outputSuffixUnsafe",
            "The output suffix contains unsafe path characters.",
        ));
    }
    Ok(())
}

fn discover_resource(
    input_index: u32,
    input: &InputResource,
    options: NormalizationOptions,
) -> Result<DiscoveryResult, AppError> {
    if input.path.trim().is_empty() {
        return Err(input_error(
            "input.path_empty",
            "errors.inputPathRequired",
            "The input path is empty.",
            Path::new(&input.path),
        ));
    }
    let raw_path = PathBuf::from(&input.path);
    let symlink_metadata = fs::symlink_metadata(&raw_path).map_err(|_| {
        input_error(
            "input.not_found",
            "errors.inputNotFound",
            "The selected input does not exist or cannot be inspected.",
            &raw_path,
        )
    })?;
    if symlink_metadata.file_type().is_symlink() {
        return Err(input_error(
            "input.symlink_unsupported",
            "errors.inputSymlinkUnsupported",
            "Symbolic links and junctions are not followed by this version.",
            &raw_path,
        ));
    }
    let canonical = fs::canonicalize(&raw_path).map_err(|_| {
        input_error(
            "input.canonicalize_failed",
            "errors.inputUnavailable",
            "The selected input path could not be normalized.",
            &raw_path,
        )
    })?;

    match input.kind {
        InputResourceKind::File => {
            if !symlink_metadata.is_file() {
                return Err(input_error(
                    "input.kind_mismatch",
                    "errors.inputKindMismatch",
                    "The selected file input is not a regular file.",
                    &canonical,
                ));
            }
            if !is_supported_input(&canonical) {
                return Err(input_error(
                    "input.extension_unsupported",
                    "errors.inputFormatUnsupported",
                    "The selected file extension is not supported by this build.",
                    &canonical,
                ));
            }
            Ok(DiscoveryResult {
                accepted: vec![InputCandidate {
                    path: canonical,
                    scan_root: None,
                }],
                had_supported_file: true,
                ..DiscoveryResult::default()
            })
        }
        InputResourceKind::Directory => {
            if !symlink_metadata.is_dir() {
                return Err(input_error(
                    "input.kind_mismatch",
                    "errors.inputKindMismatch",
                    "The selected directory input is not a directory.",
                    &canonical,
                ));
            }
            discover_directory(input_index, &canonical, input.scan_recursively, options)
        }
    }
}

fn discover_directory(
    input_index: u32,
    root: &Path,
    recursive: bool,
    options: NormalizationOptions,
) -> Result<DiscoveryResult, AppError> {
    let maximum_depth = if recursive { options.maximum_depth } else { 1 };
    let mut result = DiscoveryResult::default();
    let walker = WalkDir::new(root)
        .follow_links(false)
        .max_depth(maximum_depth)
        .sort_by_file_name();

    for entry in walker.into_iter().skip(1) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                let path = error.path().unwrap_or(root);
                result.rejected.push(rejected_input(
                    input_index,
                    &path.to_string_lossy(),
                    input_error(
                        "input.directory_entry_unavailable",
                        "errors.inputUnavailable",
                        "A directory entry could not be inspected.",
                        path,
                    ),
                ));
                continue;
            }
        };
        if entry.file_type().is_symlink() {
            continue;
        }
        if !entry.file_type().is_file() || !is_supported_input(entry.path()) {
            continue;
        }
        result.had_supported_file = true;
        let path = match fs::canonicalize(entry.path()) {
            Ok(path) => path,
            Err(_) => {
                result.rejected.push(rejected_input(
                    input_index,
                    &entry.path().to_string_lossy(),
                    input_error(
                        "input.canonicalize_failed",
                        "errors.inputUnavailable",
                        "An input file path could not be normalized.",
                        entry.path(),
                    ),
                ));
                continue;
            }
        };
        result.accepted.push(InputCandidate {
            path,
            scan_root: Some(root.to_path_buf()),
        });
    }

    Ok(result)
}

fn plan_items(
    request: &CreateJobRequest,
    candidates: Vec<InputCandidate>,
) -> Result<Vec<ItemSpec>, AppError> {
    let output_root = match &request.output.location {
        OutputLocation::SameDirectory => None,
        OutputLocation::Directory(path) => Some(normalize_output_root(Path::new(path))?),
    };
    let mut reservations = OutputReservations::new(&candidates);
    candidates
        .into_iter()
        .enumerate()
        .map(|(sequence, candidate)| {
            plan_item(
                request,
                candidate,
                output_root.as_deref(),
                sequence,
                &mut reservations,
            )
        })
        .collect()
}

fn plan_item(
    request: &CreateJobRequest,
    candidate: InputCandidate,
    output_root: Option<&Path>,
    sequence: usize,
    reservations: &mut OutputReservations,
) -> Result<ItemSpec, AppError> {
    let mut output_path = output_path_for(
        &candidate,
        output_root,
        request.output.preserve_structure,
        &request.output.suffix,
        encoder_output_extension(&request.encoder),
    )?;
    let initially_in_place = paths_equal(&candidate.path, &output_path);
    if initially_in_place && request.output.collision == CollisionPolicy::Fail {
        return Err(in_place_output_error(&output_path));
    }

    output_path =
        reservations.reserve_output(output_path, request.output.collision, &candidate.path)?;
    let in_place = paths_equal(&candidate.path, &output_path);
    if in_place && request.output.collision != CollisionPolicy::Replace {
        return Err(in_place_output_error(&output_path));
    }

    let source_backup_path = if in_place && request.output.source_backup == BackupPolicy::Enabled {
        Some(reservations.reserve_backup(backup_path_for(&candidate.path, "source-backup")?)?)
    } else {
        None
    };
    let existing_output_backup_path = if !in_place
        && request.output.collision == CollisionPolicy::Replace
        && request.output.existing_output_backup == BackupPolicy::Enabled
    {
        Some(reservations.reserve_backup(backup_path_for(&output_path, "output-backup")?)?)
    } else {
        None
    };

    Ok(ItemSpec {
        id: ItemId::default(),
        job_id: JobId::default(),
        sequence: u32::try_from(sequence).unwrap_or(u32::MAX),
        attempt: 1,
        input_path: candidate.path,
        scan_root: candidate.scan_root,
        output: OutputPlan {
            output_path,
            collision: request.output.collision,
            source_backup_path,
            existing_output_backup_path,
        },
    })
}

fn in_place_output_error(path: &Path) -> AppError {
    output_error(
        "output.in_place_requires_replace",
        "errors.inPlaceRequiresReplace",
        "In-place output requires the replace collision policy.",
        path,
        false,
    )
}

fn output_path_for(
    candidate: &InputCandidate,
    output_root: Option<&Path>,
    preserve_structure: bool,
    suffix: &str,
    extension: &str,
) -> Result<PathBuf, AppError> {
    let parent = match output_root {
        None => candidate
            .path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf(),
        Some(root) if preserve_structure => {
            let relative_parent = candidate
                .scan_root
                .as_deref()
                .and_then(|scan_root| candidate.path.strip_prefix(scan_root).ok())
                .and_then(Path::parent)
                .unwrap_or_else(|| Path::new(""));
            root.join(relative_parent)
        }
        Some(root) => root.to_path_buf(),
    };
    let stem = candidate.path.file_stem().ok_or_else(|| {
        output_error(
            "output.input_stem_missing",
            "errors.outputNameInvalid",
            "An output name could not be derived from the input file.",
            &candidate.path,
            false,
        )
    })?;
    let mut file_name = stem.to_os_string();
    file_name.push(suffix);
    let mut output = parent.join(file_name);
    output.set_extension(extension);
    Ok(output)
}

fn numbered_path(path: &Path, index: u32) -> Result<PathBuf, AppError> {
    let stem = path.file_stem().ok_or_else(|| {
        output_error(
            "output.name_invalid",
            "errors.outputNameInvalid",
            "The output file name is invalid.",
            path,
            false,
        )
    })?;
    let mut name = stem.to_os_string();
    name.push(format!(" ({index})"));
    let mut numbered = path.with_file_name(name);
    if let Some(extension) = path.extension() {
        numbered.set_extension(extension);
    }
    Ok(numbered)
}

fn backup_path_for(path: &Path, marker: &str) -> Result<PathBuf, AppError> {
    let stem = path.file_stem().ok_or_else(|| {
        output_error(
            "output.backup_name_invalid",
            "errors.backupNameInvalid",
            "A backup name could not be derived.",
            path,
            false,
        )
    })?;
    let mut name = stem.to_os_string();
    name.push(format!(".neo-rimage-{marker}"));
    if let Some(extension) = path.extension() {
        name.push(".");
        name.push(extension);
    }
    Ok(path.with_file_name(name))
}

fn normalize_output_root(path: &Path) -> Result<PathBuf, AppError> {
    let absolute = std::path::absolute(path)
        .map(|absolute| normalize_path_components(&absolute))
        .map_err(|_| output_directory_unavailable(path))?;

    if absolute.exists() {
        if !absolute.is_dir() {
            return Err(output_directory_not_directory(&absolute));
        }
        return fs::canonicalize(&absolute).map_err(|_| output_directory_unavailable(path));
    }

    // Lexical normalization removes `.`/`..` before paths are reserved. If a
    // future output directory is below a symlink/junction, resolve its deepest
    // existing ancestor now so comparisons use the same identity as inputs.
    canonicalize_existing_ancestor(&absolute).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotADirectory {
            output_directory_not_directory(path)
        } else {
            output_directory_unavailable(path)
        }
    })
}

fn canonicalize_existing_ancestor(path: &Path) -> std::io::Result<PathBuf> {
    let mut ancestor = path;
    let mut missing_components = Vec::new();
    while !ancestor.exists() {
        let component = ancestor.file_name().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "output path has no existing ancestor",
            )
        })?;
        missing_components.push(component.to_os_string());
        ancestor = ancestor.parent().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "output path has no existing ancestor",
            )
        })?;
    }

    if !fs::metadata(ancestor)?.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            "output path ancestor is not a directory",
        ));
    }
    let canonical_ancestor = fs::canonicalize(ancestor)?;
    // Avoid changing the externally visible path spelling merely because
    // Windows canonicalization adds a verbatim prefix. A genuinely redirected
    // ancestor (symlink/junction) must use its canonical target for safety.
    if same_path_spelling(&canonical_ancestor, ancestor) {
        return Ok(path.to_path_buf());
    }

    let mut resolved = canonical_ancestor;
    for component in missing_components.into_iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

fn normalize_path_components(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                // `path` is absolute, so popping at its root simply leaves the
                // root in place instead of allowing traversal above it.
                normalized.pop();
            }
        }
    }
    normalized
}

#[cfg(windows)]
fn same_path_spelling(left: &Path, right: &Path) -> bool {
    windows_path_spelling(&left.to_string_lossy())
        .eq_ignore_ascii_case(&windows_path_spelling(&right.to_string_lossy()))
}

#[cfg(windows)]
fn windows_path_spelling(path: &str) -> String {
    const VERBATIM_UNC_PREFIX: &str = r"\\?\UNC\";
    const VERBATIM_PREFIX: &str = r"\\?\";

    if path
        .get(..VERBATIM_UNC_PREFIX.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(VERBATIM_UNC_PREFIX))
    {
        return format!(r"\\{}", &path[VERBATIM_UNC_PREFIX.len()..]);
    }
    if path
        .get(..VERBATIM_PREFIX.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(VERBATIM_PREFIX))
    {
        return path[VERBATIM_PREFIX.len()..].to_owned();
    }
    path.to_owned()
}

#[cfg(not(windows))]
fn same_path_spelling(left: &Path, right: &Path) -> bool {
    left == right
}

fn output_directory_unavailable(path: &Path) -> AppError {
    output_error(
        "output.directory_unavailable",
        "errors.outputDirectoryUnavailable",
        "The output directory could not be normalized.",
        path,
        true,
    )
}

fn output_directory_not_directory(path: &Path) -> AppError {
    output_error(
        "output.directory_not_directory",
        "errors.outputDirectoryInvalid",
        "The selected output location is not a directory.",
        path,
        false,
    )
}

fn is_supported_input(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "bmp"
                    | "dib"
                    | "ff"
                    | "farbfeld"
                    | "hdr"
                    | "jpeg"
                    | "jpg"
                    | "jxl"
                    | "png"
                    | "pnm"
                    | "ppm"
                    | "psd"
                    | "qoi"
                    | "webp"
                    | "avif"
                    | "tif"
                    | "tiff"
                    | "svg"
                    | "svgz"
            )
        })
}

fn discovery_rejected_error(rejected: &[RejectedInput]) -> AppError {
    let mut error = AppError::new(
        "input.discovery_rejected",
        ErrorCategory::Input,
        "errors.inputDiscoveryRejected",
        "One or more selected inputs could not be accepted.",
    )
    .with_message_arg("rejectedCount", rejected.len().to_string());
    if let Some(first) = rejected.first() {
        error.context.path = Some(first.path.clone());
        error = error.with_message_arg("firstErrorCode", first.error.code.0.clone());
    }
    error
}

fn rejected_input(input_index: u32, path: &str, error: AppError) -> RejectedInput {
    RejectedInput {
        input_index,
        path: path.to_owned(),
        error,
    }
}

fn input_error(code: &str, message_key: &str, fallback_message: &str, path: &Path) -> AppError {
    AppError::new(code, ErrorCategory::Input, message_key, fallback_message).with_context(
        ErrorContext {
            path: Some(path.to_string_lossy().into_owned()),
            ..ErrorContext::default()
        },
    )
}

fn output_error(
    code: &str,
    message_key: &str,
    fallback_message: &str,
    path: &Path,
    retryable: bool,
) -> AppError {
    AppError::new(code, ErrorCategory::Output, message_key, fallback_message)
        .with_retryable(retryable)
        .with_context(ErrorContext {
            path: Some(path.to_string_lossy().into_owned()),
            ..ErrorContext::default()
        })
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    path_identity(left) == path_identity(right)
}

#[cfg(windows)]
type PathIdentity = String;
#[cfg(not(windows))]
type PathIdentity = PathBuf;

#[cfg(windows)]
fn path_identity(path: &Path) -> PathIdentity {
    path.as_os_str().to_string_lossy().to_lowercase()
}

#[cfg(not(windows))]
fn path_identity(path: &Path) -> PathIdentity {
    path.to_path_buf()
}

#[cfg(all(test, windows))]
mod tests {
    use super::same_path_spelling;
    use std::path::Path;

    #[test]
    fn verbatim_unc_and_standard_unc_have_the_same_spelling() {
        assert!(same_path_spelling(
            Path::new(r"\\?\UNC\server\share\future"),
            Path::new(r"\\server\share\future"),
        ));
    }
}
