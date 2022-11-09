//! Keyboard HID device implementation.

use usb_device::class_prelude::*;
use usbd_hid::descriptor::generator_prelude::*;
use usbd_hid::hid_class::{HIDClass, ReportType};

/// A trait to manage keyboard LEDs.
///
/// `()` implements this trait if you don't care of LEDs.
pub trait Leds {
    /// Sets the num lock state.
    fn num_lock(&mut self, _status: bool) {}
    /// Sets the caps lock state.
    fn caps_lock(&mut self, _status: bool) {}
    /// Sets the scroll lock state.
    fn scroll_lock(&mut self, _status: bool) {}
    /// Sets the compose state.
    fn compose(&mut self, _status: bool) {}
    /// Sets the kana state.
    fn kana(&mut self, _status: bool) {}
}
impl Leds for () {}

/// Keyboard report compatible with Boot Keyboard
///
/// A standard HID report compatible with Boot Keyboard (see HID specification, Appendix B).
/// It can handle all modifier keys and up to 6 keys pressed at the same time.
#[gen_hid_descriptor(
    (collection = APPLICATION, usage_page = GENERIC_DESKTOP, usage = KEYBOARD) = {
        (usage_page = KEYBOARD, usage_min = 0xe0, usage_max = 0xe7) = {
            #[packed_bits 8] #[item_settings data,variable,absolute] modifier = input;
        };
        (usage_min = 0x00, usage_max = 0xff) = {
            #[item_settings constant,variable,absolute] reserved=input;
        };
        (usage_page = LEDS, usage_min = 0x01, usage_max = 0x05) = {
            #[packed_bits 5] #[item_settings data,variable,absolute] leds = output;
        };
        // It would make sense to use usage_max=0xdd but boot keyboard uses 0xff. This way
        // keycodes >= KeyCode::LCtrl (notably - "unofficial media") should still work
        // (though these only work on linux, we should use different usage page for media).
        (usage_page = KEYBOARD, usage_min = 0x00, usage_max = 0xff) = {
            #[item_settings data,array,absolute] keycodes = input;
        };
    }
)]
#[derive(Default, Eq, PartialEq)]
pub struct KeyboardReport {
    /// Modifier keys packed bits
    pub modifier: u8,
    /// Boot keyboard reserved field
    pub reserved: u8,
    /// LED states (host -> device)
    pub leds: u8,
    /// Boot keyboard keycodes list
    pub keycodes: [u8; 6],
}

const KEYBOARD_REPORT_IN_SIZE: usize = 1 + 1 + 6; // all fields besides leds

/// A keyboard HID device.
pub struct HidKeyboard<'a, B: UsbBus, L> {
    hid: HIDClass<'a, B>,
    leds: L,
}

impl<'a, B: UsbBus, L> HidKeyboard<'a, B, L> {
    /// Creates a new `Keyboard` object.
    pub fn new(bus: &'a UsbBusAllocator<B>, leds: L) -> HidKeyboard<'a, B, L> {
        use usbd_hid::hid_class::*;
        let settings = HidClassSettings {
            subclass: HidSubClass::Boot,
            protocol: HidProtocol::Keyboard,
            config: ProtocolModeConfig::ForceBoot,
            locale: HidCountryCode::NotSupported,
        };
        let hid = HIDClass::new_ep_in_with_settings(bus, KeyboardReport::desc(), 10, settings);
        HidKeyboard {
            hid,
            leds,
        }
    }

    /// Push keyboard report to endpoint.
    pub fn push_keyboard_report(&mut self, report: &KeyboardReport) -> usb_device::Result<()> {
        self.hid.push_input(report)
            .and_then(|bytes_written| {
                // If bytes_written is different than report size then this means that the allocated
                // endpoint size is too small, which should be a panic!
                if bytes_written != KEYBOARD_REPORT_IN_SIZE {
                    Err(usb_device::UsbError::BufferOverflow)
                } else {
                    Ok(())
                }
            })
    }

    /// Returns the underlying leds object.
    pub fn leds_mut(&mut self) -> &mut L {
        &mut self.leds
    }
}


impl<B: UsbBus, L: Leds> UsbClass<B> for HidKeyboard<'_, B, L> {
    // Call appropriate methods from Leds on set_report request.
    fn control_out(&mut self, xfer: usb_device::class_prelude::ControlOut<B>) {
        self.hid.control_out(xfer);

        let mut leds = 0u8;
        if let Ok(info) = self.hid.pull_raw_report(core::slice::from_mut(&mut leds)) {
            if info.report_type == ReportType::Output && info.report_id == 0 && info.len == 1 {
                let bit = |i: usize| (leds & (1 << i)) != 0;
                self.leds.num_lock(bit(0));
                self.leds.caps_lock(bit(1));
                self.leds.scroll_lock(bit(2));
                self.leds.compose(bit(3));
                self.leds.kana(bit(4));
            }
        }
    }

    // Deletage all other methods to self.hid

    fn get_configuration_descriptors(&self, writer: &mut usb_device::descriptor::DescriptorWriter) -> usb_device::Result<()> {
        self.hid.get_configuration_descriptors(writer)
    }

    fn get_bos_descriptors(&self, writer: &mut usb_device::descriptor::BosWriter) -> usb_device::Result<()> {
        self.hid.get_bos_descriptors(writer)
    }

    fn get_string(&self, index: usb_device::class_prelude::StringIndex, lang_id: u16) -> Option<&str> {
        self.hid.get_string(index, lang_id)
    }

    fn reset(&mut self) {
        self.hid.reset()
    }

    fn poll(&mut self) {
        self.hid.poll()
    }

    fn control_in(&mut self, xfer: usb_device::class_prelude::ControlIn<B>) {
        self.hid.control_in(xfer)
    }

    fn endpoint_setup(&mut self, addr: usb_device::endpoint::EndpointAddress) {
        self.hid.endpoint_setup(addr)
    }

    fn endpoint_out(&mut self, addr: usb_device::endpoint::EndpointAddress) {
        self.hid.endpoint_out(addr)
    }

    fn endpoint_in_complete(&mut self, addr: usb_device::endpoint::EndpointAddress) {
        self.hid.endpoint_in_complete(addr)
    }
}
