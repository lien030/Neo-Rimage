use crossbeam_channel::Sender;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::thread::JoinHandle;

#[derive(Serialize_repr, Deserialize_repr, Debug, Clone, Copy)]
#[repr(u8)]
pub enum TaskStatus {
    Idle = 0,
    Processing = 1,
    Done = 2,
    Error = 3,
}

#[derive(Serialize_repr, Deserialize_repr, Debug, Clone, Copy)]
#[repr(u8)]
pub enum EncoderType {
    Avif = 0,
    Farbfeld = 1,
    Mozjpeg = 2,
    Jpeg = 3,
    JpegXl = 4,
    Oxipng = 5,
    Png = 6,
    Ppm = 7,
    Qoi = 8,
    Webp = 9,
}

#[derive(Serialize_repr, Deserialize_repr, Debug, Clone, Copy)]
#[repr(u8)]
pub enum FilterType {
    Nearest = 0,
    Bilinear = 1,
    Hamming = 2,
    CatmullRom = 3,
    Mitchell = 4,
    Lanczos3 = 5,
}

#[derive(Serialize_repr, Deserialize_repr, Debug, Clone, Copy)]
#[repr(u8)]
pub enum WorkerStatus {
    Idle = 0,
    Busy = 1,
    Done = 2,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Task {
    pub id: String,
    pub status: TaskStatus,
    pub file_name: String,
    pub file_path: String,
}

pub struct ProcessWorker {
    pub id: String,
    pub status: WorkerStatus,
    pub handle: JoinHandle<()>,
    pub control: Sender<()>,
    pub task: Option<Task>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SimpleWorker {
    pub id: String,
    pub status: usize,
    pub task_id: String,
}
