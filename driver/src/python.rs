use crate::Error;
use crate::{Frame, Interface};
use crossbeam_channel::{unbounded, Receiver, RecvTimeoutError, Sender};
use pyo3::exceptions;
use pyo3::prelude::*;
use pyo3::types::PyDict;

#[pyclass(name = Interface)]
struct PyInterface {
    i: Interface,
    rx_recv: Receiver<Frame>,
    rx_send: Sender<Frame>,
}

/// Return the Length to DLC.
fn raw_dlc_to_scaled_down_dlc(len : u8) -> u8 {
    match len {
        0..=8   => len,
        9..=12  => 9,
        13..=16 => 10,
        17..=20 => 11,
        21..=24 => 12,
        25..=32 => 13,
        33..=48 => 14,
        49..=64 => 15,
        65..=u8::MAX => 0,
    }
}

/// Return the DLC to length.
fn scaled_down_dlc_to_raw_dlc(dlc : u8) -> u8 {
    match dlc {
        0..=8 => dlc,
        9  => 12,
        10 => 16,
        11 => 20,
        12 => 24,
        13 => 32,
        14 => 48,
        15 => 64,
        16..=u8::MAX => 0,
    }

}

impl IntoPy<PyObject> for Frame {
    fn into_py(self, py: Python) -> PyObject {
        let d = PyDict::new(py);
        d.set_item("id", self.can_id).unwrap();
        d.set_item("dlc", self.can_dlc).unwrap();
        d.set_item("data", self.data.to_vec()).unwrap();
        d.set_item("extended", self.ext).unwrap();
        d.set_item("rtr", self.rtr).unwrap();
        d.set_item("channel", self.channel).unwrap();
        d.set_item("loopback", self.loopback).unwrap();
        match self.timestamp {
            Some(t) => d
                .set_item("timestamp", t.as_micros() as f32 / 1000000.0)
                .unwrap(),
            None => d.set_item("timestamp", 0).unwrap(),
        };
        d.to_object(py)
    }
}

impl std::convert::From<Error> for PyErr {
    fn from(err: Error) -> PyErr {
        PyErr::new::<exceptions::SystemError, _>(format!("{:?}", err))
    }
}

#[pymethods]
impl PyInterface {
    #[new]
    fn new(dev_serial_num: String) -> PyResult<Self> {
        let mut i = Interface::new(dev_serial_num)?;

        // disable all channels by default
        for n in 0..i.channels.len() {
            i.set_enabled(n, false)?;
        }

        let (send, recv) = unbounded();
        Ok(PyInterface {
            i,
            rx_recv: recv,
            rx_send: send,
        })
    }

    fn set_bitrate(&mut self, channel: usize, bitrate: u32) -> PyResult<()> {
        self.i.set_bitrate(channel, bitrate)?;
        Ok(())
    }

    fn set_data_bitrate(&mut self, channel: usize, bitrate: u32) -> PyResult<()> {
        self.i.set_data_bitrate(channel, bitrate)?;
        Ok(())
    }

    fn set_bit_timing(
        &mut self,
        channel: usize,
        brp: u32,
        phase_seg1: u32,
        phase_seg2: u32,
        sjw: u32,
    ) -> PyResult<()> {
        self.i
            .set_bit_timing(channel, brp, phase_seg1, phase_seg2, sjw)?;
        Ok(())
    }


    fn set_data_bit_timing(
        &mut self,
        channel: usize,
        brp: u32,
        phase_seg1: u32,
        phase_seg2: u32,
        sjw: u32,
    ) -> PyResult<()> {
        self.i
            .set_data_bit_timing(channel, brp, phase_seg1, phase_seg2, sjw)?;
        Ok(())
    }

    fn set_enabled(&mut self, channel: usize, enabled: bool) -> PyResult<()> {
        self.i.set_enabled(channel, enabled)?;
        Ok(())
    }

    fn set_monitor(&mut self, channel: usize, enabled: bool) -> PyResult<()> {
        self.i.set_monitor(channel, enabled)?;
        Ok(())
    }

    fn start(&mut self, channel: usize) -> PyResult<()> {
        let rx = self.rx_send.clone();

        self.i.start(channel, move |f: Frame| {
            match rx.send(f) {
                Ok(_) => {}
                Err(_) => {
                    panic!("error starting device")
                }
            };
        })?;

        Ok(())
    }

    fn stop(&mut self, channel: usize) -> PyResult<()> {
        self.i.stop(channel)?;
        Ok(())
    }

    fn recv(&self, timeout_ms: u64) -> PyResult<Option<Frame>> {
        let f = match self
            .rx_recv
            .recv_timeout(std::time::Duration::from_millis(timeout_ms))
        {
            Ok(f) => f,
            Err(RecvTimeoutError::Timeout) => return Ok(None),
            Err(RecvTimeoutError::Disconnected) => panic!("device thread died"),
        };
        Ok(Some(f))
    }

    fn send(
        &mut self,
        channel: u8,
        id: u32,
        ext: bool,
        rtr: bool,
        dlc: u8,
        data: Vec<u8>,
    ) -> PyResult<()> {
        let mut data_array: Vec<u8> = vec![0; 64];
        for i in 0..dlc as usize {
            if i < dlc.into() {
                data_array[i] = data[i];
            }
        }
        self.i.send(Frame {
            can_id: id,
            can_dlc: dlc,
            ext,
            rtr,
            data: data_array,
            channel,
            loopback: false,
            fd: false,
            brs: false,
            err: false,
            esi: false,
            timestamp: None,
        })?;
        Ok(())
    }

    fn send_fd(
        &mut self,
        channel: u8,
        id: u32,
        ext: bool,
        rtr: bool,
        fd: bool,
        brs: bool,
        dlc: u8,
        data: Vec<u8>,
    ) -> PyResult<()> {
        let mut data_array: Vec<u8> = vec![0; 64];

        if raw_dlc_to_scaled_down_dlc(dlc) == 0 {
           // Invalid Frame Size Send_fd stop here!
            return Ok(());
        }

        for i in 0..dlc as usize {
            if i < data.len() {
                data_array[i] = data[i];
            }
        }

        self.i.send(Frame {
            can_id: id,
            can_dlc: raw_dlc_to_scaled_down_dlc(dlc),
            ext,
            rtr,
            data: data_array,
            channel,
            loopback: false,
            fd,
            brs,
            err: false,
            esi: false,
            timestamp: None,
        })?;
        Ok(())
    }

    fn channel_count(&self) -> PyResult<usize> {
        Ok(self.i.channels())
    }

    fn set_fd(&mut self, channel: usize, enabled: bool) -> PyResult<()> {
        self.i.set_fd(channel, enabled)?;
        Ok(())
    }
}

#[pymodule]
fn cantact(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyInterface>()?;
    Ok(())
}
