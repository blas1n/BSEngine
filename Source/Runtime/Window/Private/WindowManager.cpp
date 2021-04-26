#ifdef WINDOWS

#define WIN32_LEAN_AND_MEAN
#define NOMINMAX

#include "WindowManager.h"
#include "Engine.h"
#include <windows.h>

namespace
{
    WindowManager* self = nullptr;
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

int32 WindowManager::Init() noexcept
{
    hInstance = GetModuleHandle(nullptr);

    WNDCLASSEX wc;
    DEVMODE dmScreenSettings;
    memset(&wc, 0, sizeof(wc));

    wc.style = CS_HREDRAW | CS_VREDRAW | CS_OWNDC | CS_DBLCLKS;
    wc.lpfnWndProc = &MsgProc;
    wc.cbClsExtra = 0;
    wc.cbWndExtra = 0;
    wc.hInstance = hInstance;
    wc.hIcon = wc.hIconSm = LoadIcon(nullptr, IDI_APPLICATION);
    wc.hCursor = nullptr;
    wc.hbrBackground = nullptr;
    wc.lpszMenuName = nullptr;
    wc.lpszClassName = ClassName;
    wc.cbSize = sizeof(WNDCLASSEX);

    if (!RegisterClassEx(&wc))
        return 1;

    size.x = GetSystemMetrics(SM_CXSCREEN);
    size.y = GetSystemMetrics(SM_CYSCREEN);

    memset(&dmScreenSettings, 0, sizeof(dmScreenSettings));
    dmScreenSettings.dmSize = sizeof(dmScreenSettings);
    dmScreenSettings.dmPelsWidth = (DWORD)size.x;
    dmScreenSettings.dmPelsHeight = (DWORD)size.y;
    dmScreenSettings.dmBitsPerPel = 32;
    dmScreenSettings.dmFields = DM_BITSPERPEL | DM_PELSWIDTH | DM_PELSHEIGHT;
    ChangeDisplaySettings(&dmScreenSettings, CDS_FULLSCREEN);

    // Todo: Support window mode (not full screen)
    HWND hWnd = CreateWindow(ClassName, STRINGIFY(GAME_NAME),
        WS_CLIPSIBLINGS | WS_CLIPCHILDREN | WS_POPUP,
        size.x, size.y, 0, 0, nullptr, nullptr, hInstance, nullptr);

    if (!hWnd) return 1;
    
    ShowWindow(hWnd, SW_SHOWDEFAULT);
    SetForegroundWindow(hWnd);
    SetFocus(hWnd);

    UpdateWindow(hWnd);
    return 0;
}

void WindowManager::Update(float deltaTime) noexcept
{
    MSG msg;
    if (PeekMessage(&msg, nullptr, 0, 0, PM_REMOVE))
    {
        TranslateMessage(&msg);
        DispatchMessage(&msg);
    }

    if (msg.message == WM_QUIT)
        GetEngine()->Exit();
}

void WindowManager::Release() noexcept
{
    ShowCursor(true);

    ChangeDisplaySettings(nullptr, 0);
    DestroyWindow(hWnd);
    UnregisterClass(ClassName, hInstance);
}

#endif
