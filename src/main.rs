use audiotags::Tag;

fn main() {
    let path = std::path::Path::new("E:\\Music");

    println!("Path: {:?}", path);
    println!("Path exists: {}", path.exists());

    recurse_directory(path);
}

fn read_tags(path: &std::path::Path) {
    let tag = Tag::new().read_from_path(path);

    if tag.is_ok() {
        let tag = tag.unwrap();
        println!("{} - {}",
                 tag.artist().unwrap_or("Unknown"),
                 tag.title().unwrap_or("Unknown"));
    }
}

fn recurse_directory(path: &std::path::Path) {
    // Enumerate over the directory recursively
    for entry in path.read_dir().unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.is_dir() {
            println!("{}", path.to_str().unwrap_or(""));
            recurse_directory(&path);
        } else {
            read_tags(&path);
        }
    }
}
