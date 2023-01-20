use log::trace;
use qrcode_generator::QrCodeEcc;
use url::Url;
use web_view::*;

pub fn open(url: &Url) {
    trace!("Testing webview...");

    web_view::builder()
        .title("My Project")
        .content(Content::Url(url))
        .size(320, 600)
        .resizable(false)
        .debug(true)
        .user_data(())
        .invoke_handler(|_webview, _arg| Ok(()))
        .run()
        .unwrap();

    trace!("Test finished.");
}

pub fn display_url_as_qrcode(url: &Url) {
    let svg: String =
        qrcode_generator::to_svg_to_string(url.to_string(), QrCodeEcc::Low, 300, None::<&str>)
            .unwrap();

    let content = Content::Html(svg);
    web_view::builder()
        .title("Authenticate for PCLI2")
        .content(Content::from(content))
        .size(320, 320)
        .resizable(false)
        .debug(true)
        .user_data(())
        .invoke_handler(|_webview, _arg| Ok(()))
        .run()
        .unwrap();
}
