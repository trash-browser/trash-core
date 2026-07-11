using UI = Gpui.DotNet.Gpui;

var white = UI.Rgb(255, 255, 255);

using var root = UI.Div()
    .Flex()
    .FlexCol()
    .Gap(12)
    .Bg(0x505050)
    .Size(500)
    .JustifyCenter()
    .ItemsCenter()
    .ShadowLg()
    .Border1()
    .BorderColor(UI.Rgb(0, 0, 255))
    .Text2Xl()
    .TextColor(white)
    .ChildText("Hello, .NET!")
    .Child(
        UI.Div()
            .Flex()
            .Gap(8)
            .Child(UI.Div().Size(32).Bg(UI.Rgb(255, 0, 0)).Border1().RoundedMd().BorderColor(white))
            .Child(UI.Div().Size(32).Bg(UI.Rgb(0, 255, 0)).Border1().RoundedMd().BorderColor(white))
            .Child(UI.Div().Size(32).Bg(UI.Rgb(0, 0, 255)).Border1().RoundedMd().BorderColor(white))
            .Child(UI.Div().Size(32).Bg(UI.Rgb(255, 255, 0)).Border1().RoundedMd().BorderColor(white))
    );

UI.Run(root, width: 500, height: 500, title: "GPUI .NET");
