use base64::{engine::general_purpose::STANDARD, Engine};
use minisign_verify::{PublicKey, Signature};

// 与 tauri-plugin-updater 的 verify_signature 同一套流程：
// base64 解码 → PublicKey::decode → Signature::decode → verify
fn base64_to_string(b64: &str) -> String {
    let bytes = STANDARD.decode(b64.trim()).expect("base64 解码失败");
    String::from_utf8(bytes).expect("UTF-8 解码失败")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("用法: sigcheck <公钥文件> <签名文件> <被签文件>");
        eprintln!("  公钥文件: 内容是 base64 文本，即 tauri.conf.json 的 plugins.updater.pubkey");
        eprintln!("  签名文件: 内容是 base64 文本，即 latest.json 里的 signature 字段");
        std::process::exit(2);
    }
    let pubkey_text = std::fs::read_to_string(&args[1]).expect("读公钥文件失败");
    let sig_text = std::fs::read_to_string(&args[2]).expect("读签名文件失败");
    let data = std::fs::read(&args[3]).expect("读被签文件失败");

    let public_key = PublicKey::decode(&base64_to_string(&pubkey_text)).expect("公钥解析失败");
    let signature = Signature::decode(&base64_to_string(&sig_text)).expect("签名解析失败");

    match public_key.verify(&data, &signature, true) {
        Ok(()) => println!("VERIFIED  {}  签名有效且与公钥匹配", args[3]),
        Err(e) => {
            println!("FAILED    {}  {e}", args[3]);
            std::process::exit(1);
        }
    }
}
