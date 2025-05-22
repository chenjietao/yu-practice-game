use std::fs;
use std::path::Path;

fn main() {
    // 只在release构建时执行
    if std::env::var("PROFILE").unwrap() == "release" {
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let release_dir = Path::new(&out_dir)
            .parent().unwrap()
            .parent().unwrap()
            .parent().unwrap();
        
        let res_dir = Path::new("res");
        let target_res_dir = release_dir.join("res");
        
        // 创建目标目录
        fs::create_dir_all(&target_res_dir).unwrap();
        
        // 复制所有文件
        for entry in fs::read_dir(res_dir).unwrap() {
            let entry = entry.unwrap();
            let target_path = target_res_dir.join(entry.file_name());
            fs::copy(entry.path(), target_path).unwrap();
        }
        
        println!("cargo:rerun-if-changed=res");
    }
}
