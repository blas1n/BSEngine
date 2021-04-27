#ifdef WINDOWS

#define WIN32_LEAN_AND_MEAN
#define NOMINMAX

#include "WindowManager.h"
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

bool WindowManager::Init() noexcept
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
    wc.hbrBackground = reinterpret_cast<HBRUSH>(GetStockObject(WHITE_BRUSH)); // Temp background
    wc.lpszMenuName = nullptr;
    wc.lpszClassName = ClassName;
    wc.cbSize = sizeof(WNDCLASSEX);

    if (!RegisterClassEx(&wc))
        return false;

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
    DestroyWindow(hWnd);
    UnregisterClass(ClassName, hInstance);
}

#endif
