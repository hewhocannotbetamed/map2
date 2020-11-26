use anyhow::Result;
use tokio::prelude::*;
use futures::future::{Future, lazy};
use tokio_threadpool::ThreadPool;

use x11rb::protocol::xproto::{GetPropertyReply, ConnectionExt, Atom, Window, intern_atom, AtomEnum, Screen, ChangeWindowAttributesAux, EventMask};
use x11rb::connection::Connection;
use x11rb::x11_utils::TryParse;
use x11rb::protocol::Event::PropertyNotify;

pub struct ActiveWindowResult {
    pub(crate) class: String,
    pub(crate) instance: String,
    pub(crate) name: String,
}

pub async fn x11_test_async() -> Result<()> {
    // tokio::spawn(
    //     move || {
    //         // tokio::tokio_threadpool::blocking(|| {
    //         std::thread::sleep(std::time::Duration::from_secs(7));
    //         println!("done");
    //         Ok(2)
    //         // })
    //     }
    // );

    // let thread_pool = ThreadPool::new();
    //
    // thread_pool.spawn(lazy(|_| -> i32 {
    //     // println!("called from a worker thread");
    //     33
    // }));
    //
    // // Gracefully shutdown the threadpool
    // thread_pool.shutdown().wait().unwrap();

    Ok(())
}


pub fn x11_test() -> Result<Option<ActiveWindowResult>> {
    let (conn, screen_id) = x11rb::connect(None)?;
    let screen: &Screen = &conn.setup().roots[screen_id];
    let root: Window = screen.root;

    conn.change_window_attributes(root, &ChangeWindowAttributesAux::new()
        .event_mask(Some(EventMask::SubstructureNotify | EventMask::PropertyChange)))?;

    // println!("screen: {}x{}", screen.width_in_pixels, screen.height_in_pixels);

    let mut NET_ACTIVE_WINDOW: Atom = intern_atom(&conn, false, b"_NET_ACTIVE_WINDOW").unwrap().reply()?.atom;


    loop {
        let event = conn.wait_for_event()?;

        if let PropertyNotify(ev) = event {
            if ev.atom == NET_ACTIVE_WINDOW {
                let res = x11_get_active_window()?;
                // println!("class: {}", res.class);
                return Ok(Some(res));
            }
        }
    }
    // else {
    //     return Ok(None);
    // }

    // Ok(None)
    // }

    // Err(anyhow::Error::msg("noes"))
}

pub fn x11_get_active_window() -> Result<ActiveWindowResult> {
    let (conn, screen) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen].root;

    let mut net_active_window: Atom = intern_atom(&conn, false, b"_NET_ACTIVE_WINDOW").unwrap().reply()?.atom;
    let mut net_wm_name: Atom = intern_atom(&conn, false, b"_NET_WM_NAME").unwrap().reply()?.atom;
    let mut utf8_string: Atom = intern_atom(&conn, false, b"UTF8_STRING").unwrap().reply()?.atom;

    let focus = find_active_window(&conn, root, net_active_window)?;

    // Collect the replies to the atoms
    let (net_wm_name, utf8_string) = (net_wm_name, utf8_string);
    let (wm_class, string) = (
        AtomEnum::WM_CLASS.into(): Atom,
        AtomEnum::STRING.into(): Atom,
    );

    // Get the property from the window that we need
    let name = conn.get_property(false, focus, net_wm_name, utf8_string, 0, u32::max_value())?;
    let class = conn.get_property(false, focus, wm_class, string, 0, u32::max_value())?;
    let (_name, class) = (name.reply()?, class.reply()?);
    let (instance, class) = parse_wm_class(&class);

    let name = parse_string_property(&_name);

    Ok(ActiveWindowResult {
        class: class.to_string(),
        instance: instance.to_string(),
        name: name.to_string(),
    })
}

fn find_active_window(conn: &impl Connection, root: Window, net_active_window: Atom) -> Result<Window> {
    let window: Atom = AtomEnum::WINDOW.into();
    // let active_window = conn.get_property(false, root, net_active_window, window, 0, 1)?.reply()?;
    let active_window = conn.get_property(false, root, net_active_window, window, 0, 1)?.reply()?;
    if active_window.format == 32 && active_window.length == 1 {
        // Things will be so much easier with the next release:
        // This does active_window.value32().next().unwrap()
        Ok(u32::try_parse(&active_window.value)?.0)
    } else {
        // Query the input focus
        Ok(conn.get_input_focus()?.reply()?.focus)
    }
}

fn parse_string_property(property: &GetPropertyReply) -> &str {
    std::str::from_utf8(&property.value).unwrap_or("Invalid utf8")
}

fn parse_wm_class(property: &GetPropertyReply) -> (&str, &str) {
    if property.format != 8 {
        return ("Malformed property: wrong format", "Malformed property: wrong format");
    }
    let value = &property.value;
    // The property should contain two null-terminated strings. Find them.
    if let Some(middle) = value.iter().position(|&b| b == 0) {
        let (instance, class) = value.split_at(middle);
        // Skip the null byte at the beginning
        let mut class = &class[1..];
        // Remove the last null byte from the class, if it is there.
        if class.last() == Some(&0) {
            class = &class[..class.len() - 1];
        }
        let instance = std::str::from_utf8(instance);
        let class = std::str::from_utf8(class);
        (instance.unwrap_or("Invalid utf8"), class.unwrap_or("Invalid utf8"))
    } else {
        ("Missing null byte", "Missing null byte")
    }
}
