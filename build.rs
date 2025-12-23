fn main() {
    // Embed the Windows resource file (icon and manifest)
    embed_resource::compile("resources/app.rc", embed_resource::NONE);
}
