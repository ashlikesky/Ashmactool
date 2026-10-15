import AppKit

let project = URL(fileURLWithPath:CommandLine.arguments[1])
let data = try Data(contentsOf:project.appendingPathComponent("assets/cursors/geometric.json"))
let spec = try JSONSerialization.jsonObject(with:data) as! [String:Any]
var lightMode = false
let height:CGFloat = 560
let rep = NSBitmapImageRep(bitmapDataPlanes:nil,pixelsWide:1080,pixelsHigh:Int(height),bitsPerSample:8,
                          samplesPerPixel:4,hasAlpha:true,isPlanar:false,colorSpaceName:.deviceRGB,bytesPerRow:0,bitsPerPixel:0)!
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep:rep)
NSColor(srgbRed:0.13,green:0.14,blue:0.17,alpha:1).setFill()
NSRect(x:0,y:0,width:1080,height:height).fill()
func text(_ value:String,_ x:CGFloat,_ y:CGFloat,_ size:CGFloat,_ dim:Bool=false) {
    let attrs:[NSAttributedString.Key:Any] = [.font:NSFont.systemFont(ofSize:size),.foregroundColor:dim ? (lightMode ? NSColor(calibratedWhite:0.42,alpha:1) : NSColor(srgbRed:0.63,green:0.67,blue:0.74,alpha:1)) : (lightMode ? NSColor.black : NSColor.white)]
    let extent=(value as NSString).size(withAttributes:attrs)
    (value as NSString).draw(at:NSPoint(x:x,y:height-y-extent.height),withAttributes:attrs)
}
func path(_ source:String) -> NSBezierPath {
    let regex=try! NSRegularExpression(pattern:"[MLCZ]|[-+]?[0-9]*\\.?[0-9]+")
    let ns=source as NSString
    let tokens=regex.matches(in:source,range:NSRange(location:0,length:ns.length)).map{ns.substring(with:$0.range)}
    let p=NSBezierPath();var i=0
    func n()->CGFloat {defer{i+=1};return CGFloat(Double(tokens[i])!)}
    while i<tokens.count {
        let op=tokens[i];i+=1
        switch op {
        case "M":p.move(to:NSPoint(x:n(),y:n()))
        case "L":p.line(to:NSPoint(x:n(),y:n()))
        case "C":let a=NSPoint(x:n(),y:n()),b=NSPoint(x:n(),y:n()),c=NSPoint(x:n(),y:n());p.curve(to:c,controlPoint1:a,controlPoint2:b)
        case "Z":p.close()
        default:fatalError("Unsupported path command")
        }
    }
    p.lineCapStyle = .round;p.lineJoinStyle = .round
    return p
}
func glyph(_ icon:[String:Any],_ x:CGFloat,_ y:CGFloat,_ side:CGFloat,_ stroke:CGFloat,_ blur:CGFloat) {
    NSGraphicsContext.saveGraphicsState()
    defer { NSGraphicsContext.restoreGraphicsState() }
    let context=NSGraphicsContext.current!.cgContext
    let scale=side/64
    context.translateBy(x:x,y:height-y)
    context.scaleBy(x:scale,y:-scale)
    let shadow=NSShadow();shadow.shadowColor=NSColor(srgbRed:0.64,green:0.47,blue:0.94,alpha:0.45)
    shadow.shadowBlurRadius=blur*scale;shadow.shadowOffset = .zero;shadow.set()
    let p=path(icon["path"] as! String);p.lineWidth=stroke
    (lightMode ? NSColor.white : NSColor(srgbRed:0.025,green:0.03,blue:0.04,alpha:1)).setFill();(lightMode ? NSColor.black : NSColor.white).setStroke()
    if icon["filled"] as? Bool ?? true {p.fill()};p.stroke()
    let empty=NSShadow();empty.shadowColor = .clear;empty.set()
    for detail in icon["details"] as? [String] ?? [] {let d=path(detail);d.lineWidth=1.25;d.stroke()}
}
text("极简几何指针",28,24,26)
text("保留必要的动作特征 · 平直线段 + 少量圆角",28,65,14,true)
let names=["普通箭头参照","文本插入","链接手形","抓握","水平调整"]
let columns:[CGFloat]=[170,344,518,692,866]
for (i,name) in names.enumerated(){text(name,columns[i]+6,110,14,true)}
for row in 0..<2 {
    let y:CGFloat=row==0 ? 158:358
    if row==1 {
        NSColor(srgbRed:0.96,green:0.96,blue:0.97,alpha:1).setFill()
        NSBezierPath(roundedRect:NSRect(x:16,y:height-516,width:1048,height:180),xRadius:14,yRadius:14).fill()
    }
    lightMode = row==1
    text(row==0 ? "深色外观":"浅色外观",24,y+16,17)
    text("几何化轮廓",24,y+48,12,true)
    text("统一 2 pt 线条",24,y+68,12,true)
    let icons=[["path":spec["reference"]!,"details":[]] as [String:Any]]+(spec["icons"] as! [[String:Any]])
    for (i,icon) in icons.enumerated() {
        glyph(icon,columns[i],y,88,i==0 ? 2.5:2,i==0 ? 3:0)
        glyph(icon,columns[i]+30,y+108,28,i==0 ? 2.5:2,i==0 ? 3:0)
    }
}
lightMode=false
text("大图看造型，下方小图按 28 点画布显示。",28,538,12,true)
let output=project.appendingPathComponent("previews/cursor-minimal-direction.png")
try rep.representation(using:.png,properties:[:])!.write(to:output)
print(output.path)
