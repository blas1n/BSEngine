#ifdef WINDOWS

#define WIN32_LEAN_AND_MEAN
#define NOMINMAX

#include "WindowManager.h"
#include <windows.h>

namespace
{
    constexpr char* ClassName = "BSEngine";

    LRESULT WINAPI MsgProc(HWND hWnd, UINT msg, WPARAM wParam, LPARAM lParam)
    {
        switch (msg)
        {
        case WM_DESTROY:
            PostQuitMessage(0);
            break;
        }

        return DefWindowProc(hWnd, msg, wParam, lParam);
    }
}

struct Handle final
{
    HINSTANCE hInstance = nullptr;
    HWND hWnd = nullptr;
};

bool WindowManager::Init() noexcept
{
    handle = new Handle;
    handle->hInstance = GetModuleHandle(nullptr);

    WNDCLASSEX wc;
    DEVMODE dmScreenSettings;
    memset(&wc, 0, sizeof(wc));

    wc.style = CS_HREDRAW | CS_VREDRAW | CS_OWNDC | CS_DBLCLKS;
    wc.lpfnWndProc = &MsgProc;
    wc.cbClsExtra = 0;
    wc.cbWndExtra = 0;
    wc.hInstance = handle->hInstance;
    wc.hIcon = wc.hIconSm = LoadIcon(nullptr, IDI_APPLICATION);
    wc.hCursor = nullptr;
    wc.hbrBackground = nullptr;
    wc.lpszMenuName = nullptr;
    wc.lpszClassName = ClassName;
    wc.cbSize = sizeof(WNDCLASSEX);

    if (!RegisterClassEx(&wc))
        return false;

    size.x = GetSystemMetrics(SM_CXSCREEN);
    size.y = GetSystemMetrics(SM_CYSCREEN);

    memset(&dmScreenSettings, 0, sizeof(dmScreenSettings));
    dmScreenSettings.dmSize = sizeof(dmScreenSettings);
    dmScreenSettings.dmPelsWidth = static_cast<DWORD>(size.x);
    dmScreenSettings.dmPelsHeight = static_cast<DWORD>(size.y);
    dmScreenSettings.dmBitsPerPel = 32;
    dmScreenSettings.dmFields = DM_BITSPERPEL | DM_PELSWIDTH | DM_PELSHEIGHT;
    ChangeDisplaySettings(&dmScreenSettings, CDS_FULLSCREEN);

    /// @todo: Support window mode (not full screen)

    const std::string nameStr = CastCharSet<char>(StringView{ gameName.ToString().c_str() });
    HWND hWnd = CreateWindowEx(WS_EX_APPWINDOW, ClassName, nameStr.c_str(),
        WS_CLIPSIBLINGS | WS_CLIPCHILDREN | WS_POPUP,
        0, 0, size.x, size.y, nullptr, nullptr, handle->hInstance, nullptr);

    if (!hWnd) return false;
    
    ShowWindow(hWnd, SW_SHOWDEFAULT);
    SetForegroundWindow(hWnd);
    SetFocus(hWnd);

    UpdateWindow(hWnd);
    return true;
}

bool WindowManager::Update(float deltaTime) noexcept
{
    MSG msg;
    if (PeekMessage(&msg, nullptr, 0, 0, PM_REMOVE))
    {
        TranslateMessage(&msg);
        DispatchMessage(&msg);
    }

    return msg.message != WM_QUIT;
}

void WindowManager::Release() noexcept
{
    ShowCursor(true);

    ChangeDisplaySettings(nullptr, 0);
    DestroyWindow(handle->hWnd);
    UnregisterClass(ClassName, handle->hInstance);
    delete handle;
}

#endif
