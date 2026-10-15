// Development preview renderer; the application remains a native Rust binary.
import AppKit

let args = CommandLine.arguments
guard args.count >= 3 else { fatalError("Usage: render-cursor-sheet.swift INPUT OUTPUT [BEFORE COMPARISON]") }
let source = URL(fileURLWithPath: args[1])
let destination = URL(fileURLWithPath: args[2])
let labels: [(Int, String)] = [
    (0,"箭头"),(1,"文字插入"),(2,"拖动链接"),(3,"禁止操作"),(4,"背景忙碌"),
    (5,"拖动复制"),(7,"十字线"),(8,"十字线（细）"),(9,"截图相机（小）"),(10,"截图相机"),
    (11,"抓握"),(12,"抓取"),(13,"链接指示"),(14,"计数上升"),(15,"计数下降"),
    (16,"双向计数"),(17,"向左调整"),(18,"向右调整"),(19,"水平调整"),(20,"单元格选取"),
    (21,"向上调整"),(22,"向下调整"),(23,"垂直调整"),(24,"上下文菜单"),(25,"消失操作"),
    (26,"纵向文字插入"),(27,"窗口右边"),(28,"窗口水平调整"),(29,"窗口右上角"),
    (30,"窗口右上／左下"),(31,"窗口上边"),(32,"窗口垂直调整"),(33,"窗口左上角"),
    (34,"窗口左上／右下"),(35,"窗口右下角"),(36,"窗口下边"),(37,"窗口左下角"),
    (38,"窗口左边"),(39,"移动"),(40,"帮助"),(41,"单元格"),(42,"放大"),(43,"缩小")
]
let items = labels.map { ("com.apple.cursor.\($0.0)", $0.1) }
    .filter { FileManager.default.fileExists(atPath: source.appendingPathComponent("glow/\($0.0).png").path) }
    + [("com.apple.coregraphics.Wait", "系统等待／沙滩球")]
let styles = [("glow","柔光",NSColor(srgbRed:0.15,green:0.16,blue:0.20,alpha:1)),
              ("dark","黑色",NSColor.white),
              ("light","白色",NSColor(srgbRed:0.15,green:0.16,blue:0.20,alpha:1))]
func render(_ width: Int, _ height: Int, _ output: URL, _ draw: (CGFloat) throws -> Void) throws {
    let rep = NSBitmapImageRep(bitmapDataPlanes:nil,pixelsWide:width,pixelsHigh:height,bitsPerSample:8,
                              samplesPerPixel:4,hasAlpha:true,isPlanar:false,colorSpaceName:.deviceRGB,
                              bytesPerRow:0,bitsPerPixel:0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep:rep)
    defer { NSGraphicsContext.restoreGraphicsState() }
    try draw(CGFloat(height))
    try rep.representation(using:.png,properties:[:])!.write(to:output)
}
func rect(_ x: CGFloat,_ y: CGFloat,_ w: CGFloat,_ h: CGFloat,_ height: CGFloat) -> NSRect {
    NSRect(x:x,y:height-y-h,width:w,height:h)
}
func text(_ value: String,_ x: CGFloat,_ y: CGFloat,_ size: CGFloat,_ height: CGFloat,_ color: NSColor = NSColor(srgbRed:0.10,green:0.12,blue:0.16,alpha:1)) {
    let attrs: [NSAttributedString.Key:Any] = [.font:NSFont.systemFont(ofSize:size),.foregroundColor:color]
    let extent = (value as NSString).size(withAttributes:attrs)
    (value as NSString).draw(at:NSPoint(x:x,y:height-y-extent.height),withAttributes:attrs)
}
func image(_ folder: URL,_ style: String,_ name: String,_ frame: NSRect) throws {
    let path = folder.appendingPathComponent("\(style)/\(name).png")
    guard let image = NSImage(contentsOf:path) else { throw NSError(domain:"CursorPreview",code:1,userInfo:[NSLocalizedDescriptionKey:path.path]) }
    image.draw(in:frame,from:.zero,operation:.sourceOver,fraction:1,respectFlipped:false,hints:nil)
}
let rows = (items.count+1)/2
try render(1120,142+rows*80,destination) { height in
    NSColor(srgbRed:0.97,green:0.97,blue:0.98,alpha:1).setFill()
    NSRect(x:0,y:0,width:1120,height:height).fill()
    text("Ashmactool · 整套指针主题",28,22,30,height)
    text("28 / 35 / 45.5 / 56 点 · 按可见轮廓统一视觉尺寸",28,66,17,height,NSColor(srgbRed:0.40,green:0.44,blue:0.49,alpha:1))
    for column in 0..<2 { for (i,style) in styles.enumerated() {
        text(style.1,CGFloat(24+column*560+242+i*94),98,13,height)
    }}
    for (index,item) in items.enumerated() {
        let x = CGFloat(24+index/rows*560), y = CGFloat(125+index%rows*80)
        text(item.1,x,y+27,17,height)
        for (i,style) in styles.enumerated() {
            let sx = x+224+CGFloat(i*94)
            style.2.setFill()
            NSBezierPath(roundedRect:rect(sx,y,86,72,height),xRadius:10,yRadius:10).fill()
            try image(source,style.0,item.0,rect(sx+11,y+4,64,64,height))
        }
    }
    text("\(items.count) 个预览项目 / 53 项系统指针定义（含别名）· 动画展示首帧",28,height-20,13,height,NSColor(srgbRed:0.40,green:0.44,blue:0.49,alpha:1))
}
if args.count >= 5 {
    let before = URL(fileURLWithPath:args[3]), comparison = URL(fileURLWithPath:args[4])
    let examples = [(0,"普通箭头"),(1,"文本插入"),(13,"链接手形"),(11,"抓握"),(19,"水平调整")]
    try render(720,260,comparison) { height in
        NSColor(srgbRed:0.15,green:0.16,blue:0.20,alpha:1).setFill()
        NSRect(x:0,y:0,width:720,height:height).fill()
        text(args.count >= 6 ? args[5] : "普通箭头不变 · 其他状态按可见轮廓放大",24,16,18,height,.white)
        for (i,example) in examples.enumerated() { text(example.1,CGFloat(142+i*112),55,14,height,.white) }
        for (row,folder) in [before,source].enumerated() {
            let y = CGFloat(84+row*84)
            text(row == 0 ? "调整前" : "调整后",24,y+22,17,height,.white)
            for (i,example) in examples.enumerated() {
                try image(folder,"glow","com.apple.cursor.\(example.0)",rect(CGFloat(140+i*112),y,64,64,height))
            }
        }
    }
}
print(destination.path)
