use core::mem;
use std::sync::mpsc;

use ::oneshot;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict};
use writer::EventRoute;

use crate::*;
use crate::writer::EventRoutable;

#[pyclass]
pub struct Reader {
    exit_tx: Option<oneshot::Sender<()>>,
    thread_handle: Option<std::thread::JoinHandle<Result<()>>>,
    ev_rx: Option<mpsc::Receiver<InputEvent>>,
}


#[pyclass]
pub struct RoutableObj {
    pub(crate) inner: Result<mpsc::Receiver<InputEvent>>,
}

impl FromPyObject<'a> for RoutableObj {
    fn extract(ob: &'a PyAny) -> PyResult<Self> {
        if let Ok(foo) = ob.downcast_ref::<RoutableObj>() {
            Ok(PyMap(dict.items().extract::<Vec<(K, V)>>()?.into_iter().collect::<HashMap<_,_>>()))
        } else {
            panic!();
        }
    }
}


#[pymethods]
impl Reader {
    #[new]
    #[args(kwargs = "**")]
    pub fn new(kwargs: Option<&PyDict>) -> PyResult<Self> {
        let options: HashMap<&str, &PyAny> = kwargs
            .ok_or_else(|| PyTypeError::new_err("no options provided"))?
            .extract()
            .map_err(|_| PyTypeError::new_err("the options object must be a dict"))?;

        let patterns: Vec<&str> = options.get("patterns")
            .ok_or_else(|| PyTypeError::new_err("'patterns' is required but was not provided"))?
            .extract()
            .map_err(|_| PyTypeError::new_err("'patterns' must be a list"))?;

        let (exit_tx, exit_rx) = oneshot::channel();
        let (ev_tx, ev_rx) = mpsc::channel();

        let thread_handle = grab_udev_inputs(&patterns, ev_tx, exit_rx)
            .map_err(|err| PyTypeError::new_err(err.to_string()))?;

        let handle = Self {
            exit_tx: Some(exit_tx),
            thread_handle: Some(thread_handle),
            ev_rx: Some(ev_rx),
        };

        Ok(handle)
    }

    fn route(&mut self) -> RoutableObj {
        if self.ev_rx.is_none() {
            return RoutableObj { inner: Err(anyhow!("reader is already bound to an output, multiplexing is not allowed.")) };
        }
        let mut reader = None;
        mem::swap(&mut reader, &mut self.ev_rx);
        RoutableObj { inner: Ok(reader.unwrap()) }
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        let _ = self.exit_tx.take().unwrap().send(());
        let _ = self.thread_handle.take().unwrap().try_timed_join(Duration::from_millis(100)).unwrap();
    }
}
