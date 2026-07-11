using System.Runtime.InteropServices;

namespace Gpui.DotNet;

public static class Gpui
{
    public static Element Div() => new(Native.ElementNew(), ownsHandle: true);

    public static Element Text(string value) => Div().ChildText(value);

    public static uint Rgb(byte r, byte g, byte b) => Native.Rgb(r, g, b);

    public static uint Rgba(byte r, byte g, byte b, byte a) => Native.Rgba(r, g, b, a);

    public static bool Run(Element root, float width = 500, float height = 500, string? title = null)
    {
        ObjectDisposedException.ThrowIf(root.IsDisposed, root);
        return Native.Run(root.Handle, width, height, title);
    }
}

public sealed class Element : IDisposable
{
    private bool ownsHandle;

    internal Element(IntPtr handle, bool ownsHandle)
    {
        if (handle == IntPtr.Zero)
        {
            throw new InvalidOperationException("Failed to create GPUI element.");
        }

        Handle = handle;
        this.ownsHandle = ownsHandle;
    }

    internal IntPtr Handle { get; private set; }

    internal bool IsDisposed => Handle == IntPtr.Zero;

    public Element Clone()
    {
        ObjectDisposedException.ThrowIf(IsDisposed, this);
        return new Element(Native.ElementClone(Handle), ownsHandle: true);
    }

    public Element Child(Element child)
    {
        ObjectDisposedException.ThrowIf(IsDisposed, this);
        ObjectDisposedException.ThrowIf(child.IsDisposed, child);
        Native.ElementChild(Handle, child.Handle);
        return this;
    }

    public Element ChildText(string text)
    {
        ObjectDisposedException.ThrowIf(IsDisposed, this);
        Native.ElementChildText(Handle, text);
        return this;
    }

    public Element Absolute() => Style(Native.ElementAbsolute);
    public Element Bg(uint hex) => Style(hex, Native.ElementBg);
    public Element BgRgba(uint hex) => Style(hex, Native.ElementBgRgba);
    public Element Border1() => Style(Native.ElementBorder1);
    public Element BorderColor(uint hex) => Style(hex, Native.ElementBorderColor);
    public Element BorderDashed() => Style(Native.ElementBorderDashed);
    public Element Flex() => Style(Native.ElementFlex);
    public Element Flex1() => Style(Native.ElementFlex1);
    public Element FlexCol() => Style(Native.ElementFlexCol);
    public Element FlexRow() => Style(Native.ElementFlexRow);
    public Element Gap(float value) => Style(value, Native.ElementGap);
    public Element GapX(float value) => Style(value, Native.ElementGapX);
    public Element GapY(float value) => Style(value, Native.ElementGapY);
    public Element Grid() => Style(Native.ElementGrid);
    public Element Hidden() => Style(Native.ElementHidden);
    public Element Height(float value) => Style(value, Native.ElementHeight);
    public Element H(float value) => Height(value);
    public Element ItemsCenter() => Style(Native.ElementItemsCenter);
    public Element ItemsEnd() => Style(Native.ElementItemsEnd);
    public Element ItemsStart() => Style(Native.ElementItemsStart);
    public Element JustifyBetween() => Style(Native.ElementJustifyBetween);
    public Element JustifyCenter() => Style(Native.ElementJustifyCenter);
    public Element JustifyEnd() => Style(Native.ElementJustifyEnd);
    public Element JustifyStart() => Style(Native.ElementJustifyStart);
    public Element Margin(float value) => Style(value, Native.ElementMargin);
    public Element M(float value) => Margin(value);
    public Element Mx(float value) => Style(value, Native.ElementMarginX);
    public Element My(float value) => Style(value, Native.ElementMarginY);
    public Element Opacity(float value) => Style(value, Native.ElementOpacity);
    public Element Padding(float value) => Style(value, Native.ElementPadding);
    public Element P(float value) => Padding(value);
    public Element Px(float value) => Style(value, Native.ElementPaddingX);
    public Element Py(float value) => Style(value, Native.ElementPaddingY);
    public Element Relative() => Style(Native.ElementRelative);
    public Element RoundedFull() => Style(Native.ElementRoundedFull);
    public Element RoundedMd() => Style(Native.ElementRoundedMd);
    public Element ShadowLg() => Style(Native.ElementShadowLg);
    public Element Size(float value) => Width(value).Height(value);
    public Element Text2Xl() => Style(Native.ElementText2Xl);
    public Element TextColor(uint hex) => Style(hex, Native.ElementTextColor);
    public Element TextLg() => Style(Native.ElementTextLg);
    public Element TextSm() => Style(Native.ElementTextSm);
    public Element TextXl() => Style(Native.ElementTextXl);
    public Element Width(float value) => Style(value, Native.ElementWidth);
    public Element W(float value) => Width(value);

    public void Dispose()
    {
        if (Handle != IntPtr.Zero && ownsHandle)
        {
            Native.ElementFree(Handle);
        }

        Handle = IntPtr.Zero;
        ownsHandle = false;
        GC.SuppressFinalize(this);
    }

    ~Element() => Dispose();

    private Element Style(Func<IntPtr, IntPtr> apply)
    {
        ObjectDisposedException.ThrowIf(IsDisposed, this);
        apply(Handle);
        return this;
    }

    private Element Style(float value, Func<IntPtr, float, IntPtr> apply)
    {
        ObjectDisposedException.ThrowIf(IsDisposed, this);
        apply(Handle, value);
        return this;
    }

    private Element Style(uint value, Func<IntPtr, uint, IntPtr> apply)
    {
        ObjectDisposedException.ThrowIf(IsDisposed, this);
        apply(Handle, value);
        return this;
    }
}

internal static partial class Native
{
    private const string Library = "gpui_dotnet";

    [DllImport(Library, EntryPoint = "gpui_dotnet_element_new")]
    internal static extern IntPtr ElementNew();

    [DllImport(Library, EntryPoint = "gpui_dotnet_element_clone")]
    internal static extern IntPtr ElementClone(IntPtr element);

    [DllImport(Library, EntryPoint = "gpui_dotnet_element_free")]
    internal static extern void ElementFree(IntPtr element);

    [DllImport(Library, EntryPoint = "gpui_dotnet_element_child")]
    internal static extern IntPtr ElementChild(IntPtr element, IntPtr child);

    [DllImport(Library, EntryPoint = "gpui_dotnet_element_child_text")]
    internal static extern IntPtr ElementChildText(IntPtr element, [MarshalAs(UnmanagedType.LPUTF8Str)] string text);

    [DllImport(Library, EntryPoint = "gpui_dotnet_run")]
    [return: MarshalAs(UnmanagedType.I1)]
    internal static extern bool Run(IntPtr root, float width, float height, [MarshalAs(UnmanagedType.LPUTF8Str)] string? title);

    [DllImport(Library, EntryPoint = "gpui_dotnet_rgb")]
    internal static extern uint Rgb(byte r, byte g, byte b);

    [DllImport(Library, EntryPoint = "gpui_dotnet_rgba")]
    internal static extern uint Rgba(byte r, byte g, byte b, byte a);

    [DllImport(Library, EntryPoint = "gpui_dotnet_element_absolute")]
    internal static extern IntPtr ElementAbsolute(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_bg")]
    internal static extern IntPtr ElementBg(IntPtr element, uint hex);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_bg_rgba")]
    internal static extern IntPtr ElementBgRgba(IntPtr element, uint hex);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_border_1")]
    internal static extern IntPtr ElementBorder1(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_border_color")]
    internal static extern IntPtr ElementBorderColor(IntPtr element, uint hex);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_border_dashed")]
    internal static extern IntPtr ElementBorderDashed(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_flex")]
    internal static extern IntPtr ElementFlex(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_flex_1")]
    internal static extern IntPtr ElementFlex1(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_flex_col")]
    internal static extern IntPtr ElementFlexCol(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_flex_row")]
    internal static extern IntPtr ElementFlexRow(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_gap")]
    internal static extern IntPtr ElementGap(IntPtr element, float value);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_gap_x")]
    internal static extern IntPtr ElementGapX(IntPtr element, float value);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_gap_y")]
    internal static extern IntPtr ElementGapY(IntPtr element, float value);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_grid")]
    internal static extern IntPtr ElementGrid(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_hidden")]
    internal static extern IntPtr ElementHidden(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_height")]
    internal static extern IntPtr ElementHeight(IntPtr element, float value);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_items_center")]
    internal static extern IntPtr ElementItemsCenter(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_items_end")]
    internal static extern IntPtr ElementItemsEnd(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_items_start")]
    internal static extern IntPtr ElementItemsStart(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_justify_between")]
    internal static extern IntPtr ElementJustifyBetween(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_justify_center")]
    internal static extern IntPtr ElementJustifyCenter(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_justify_end")]
    internal static extern IntPtr ElementJustifyEnd(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_justify_start")]
    internal static extern IntPtr ElementJustifyStart(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_margin")]
    internal static extern IntPtr ElementMargin(IntPtr element, float value);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_margin_x")]
    internal static extern IntPtr ElementMarginX(IntPtr element, float value);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_margin_y")]
    internal static extern IntPtr ElementMarginY(IntPtr element, float value);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_opacity")]
    internal static extern IntPtr ElementOpacity(IntPtr element, float value);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_padding")]
    internal static extern IntPtr ElementPadding(IntPtr element, float value);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_padding_x")]
    internal static extern IntPtr ElementPaddingX(IntPtr element, float value);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_padding_y")]
    internal static extern IntPtr ElementPaddingY(IntPtr element, float value);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_relative")]
    internal static extern IntPtr ElementRelative(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_rounded_full")]
    internal static extern IntPtr ElementRoundedFull(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_rounded_md")]
    internal static extern IntPtr ElementRoundedMd(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_shadow_lg")]
    internal static extern IntPtr ElementShadowLg(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_text_2xl")]
    internal static extern IntPtr ElementText2Xl(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_text_color")]
    internal static extern IntPtr ElementTextColor(IntPtr element, uint hex);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_text_lg")]
    internal static extern IntPtr ElementTextLg(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_text_sm")]
    internal static extern IntPtr ElementTextSm(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_text_xl")]
    internal static extern IntPtr ElementTextXl(IntPtr element);
    [DllImport(Library, EntryPoint = "gpui_dotnet_element_width")]
    internal static extern IntPtr ElementWidth(IntPtr element, float value);
}
