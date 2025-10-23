use libc::{c_int, c_uchar};
use libusb1_sys;
use std::mem;
use std::ptr;
use std::slice;

pub struct LibUsbHelperDevice {
    vendor_id: u16,
    product_id: u16,
    serial_number: String,
    ctx: *mut libusb1_sys::libusb_context,
}

impl LibUsbHelperDevice {
    pub fn new(ctx: *mut libusb1_sys::libusb_context, vendor_id: u16, product_id: u16, serial_num: String) -> LibUsbHelperDevice {
        LibUsbHelperDevice {
            serial_number: String::from(serial_num),
            vendor_id: vendor_id,
            product_id: product_id,
            ctx: ctx,
        }
    }

    // Return the USB serial number as string
    fn get_usb_string_descriptor(&mut self, handle: *mut libusb1_sys::libusb_device_handle, desc_index: u8) -> Option<String> {
        if handle.is_null() || (0 == desc_index) {
            return None;
        }

        let mut vec = Vec::<u8>::with_capacity(256);
        let ptr = (&mut vec[..]).as_mut_ptr();

        let len = unsafe {
            libusb1_sys::libusb_get_string_descriptor_ascii(
                handle,
                desc_index,
                ptr as *mut c_uchar,
                vec.capacity() as c_int,
            )
        };

        if len > 0 {
            unsafe { vec.set_len(len as usize) };

            match String::from_utf8(vec) {
                Ok(s) => Some(s),
                Err(_) => None,
            }
        } else {
            None
        }
    }

    // Return the USB device handle
    pub fn open_device_with_vid_pid_serial(&mut self) -> *mut libusb1_sys::libusb_device_handle {
        // Creates a null mutable raw pointer.
        let mut ret_handle: *mut libusb1_sys::libusb_device_handle = ptr::null_mut();

        // <MaybeUninit> used for avoid unnecessary initialization
        let mut listdev: mem::MaybeUninit<*const *mut libusb1_sys::libusb_device> = mem::MaybeUninit::<*const *mut libusb1_sys::libusb_device>::uninit();  //list

        // Get the available USB devices
        let available_devices_count = unsafe { libusb1_sys::libusb_get_device_list(self.ctx, listdev.as_mut_ptr())};
         if available_devices_count < 0 {
            return ret_handle ;
         }
        // Convert the MaybeUninit to an libusb_device, assuming it's initialized
        let device_list = unsafe { listdev.assume_init() };

        // Raw pointer obtained through libusb (FFI) convert to &mut [T]
        let devs = unsafe { slice::from_raw_parts(device_list, available_devices_count as usize) };
        // Iterate each USB device from devs
        for dev in devs {
            let mut desc = mem::MaybeUninit::<libusb1_sys::libusb_device_descriptor>::uninit();
            let mut handle: *mut libusb1_sys::libusb_device_handle = ptr::null_mut();
            let descriptor: Option<libusb1_sys::libusb_device_descriptor> =
            // Get device descriptor
            match unsafe { libusb1_sys::libusb_get_device_descriptor(*dev, desc.as_mut_ptr()) } {
                0 => {
                    Some(unsafe { desc.assume_init() })
                },
                _  => None,
            };

            // Check if &descriptor have data if yes &descriptor assign  Some(descriptor) then block will run otherwise skip
            if let Some(descriptor) = &descriptor {
                // Compare VID and PID
                if (descriptor.idVendor == self.vendor_id) && (descriptor.idProduct == self.product_id) {
                    unsafe {
                        // Open USB device
                        if  libusb1_sys::libusb_open(*dev, &mut handle)  < 0 {
                            // Couldn't open device, some information will be missing
                            continue;
                        }

                        let mut dev_serial =  String::new();
                        if descriptor.iSerialNumber > 0 {
                            // Get the USB serial number
                            match self.get_usb_string_descriptor(handle, descriptor.iSerialNumber) {
                                Some(serial) => {
                                    dev_serial = serial;
                                },
                                None => (),
                            }
                        }
                        // Compare serial number
                        if dev_serial == self.serial_number {
                            ret_handle = handle;
                            // Free the device list memory
                            libusb1_sys::libusb_free_device_list(device_list,1);
                            break;
                        } else {
                            // When match failed close the object
                            if !handle.is_null() {
                                { libusb1_sys::libusb_close(handle) };
                            }
                        }
                    }
                }
            }
        }
        return ret_handle;
    }
}
