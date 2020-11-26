#pragma once

#include <SDL2/SDL.h>
#include "Vector2.h"

namespace ArenaBoss
{
    enum class Key
    {
        A = SDL_SCANCODE_A,
        B = SDL_SCANCODE_B,
        C = SDL_SCANCODE_C,
        D = SDL_SCANCODE_D,
        E = SDL_SCANCODE_E,
        F = SDL_SCANCODE_F,
        G = SDL_SCANCODE_G,
        H = SDL_SCANCODE_H,
        I = SDL_SCANCODE_I,
        J = SDL_SCANCODE_J,
        K = SDL_SCANCODE_K,
        L = SDL_SCANCODE_L,
        M = SDL_SCANCODE_M,
        N = SDL_SCANCODE_N,
        O = SDL_SCANCODE_O,
        P = SDL_SCANCODE_P,
        Q = SDL_SCANCODE_Q,
        R = SDL_SCANCODE_R,
        S = SDL_SCANCODE_S,
        T = SDL_SCANCODE_T,
        U = SDL_SCANCODE_U,
        V = SDL_SCANCODE_V,
        W = SDL_SCANCODE_W,
        X = SDL_SCANCODE_X,
        Y = SDL_SCANCODE_Y,
        Z = SDL_SCANCODE_Z,

        Up = SDL_SCANCODE_UP,
        Down = SDL_SCANCODE_DOWN,
        Left = SDL_SCANCODE_LEFT,
        Right = SDL_SCANCODE_RIGHT,

        Num0 = SDL_SCANCODE_0,
        Num1 = SDL_SCANCODE_1,
        Num2 = SDL_SCANCODE_2,
        Num3 = SDL_SCANCODE_3,
        Num4 = SDL_SCANCODE_4,
        Num5 = SDL_SCANCODE_5,
        Num6 = SDL_SCANCODE_6,
        Num7 = SDL_SCANCODE_7,
        Num8 = SDL_SCANCODE_8,
        Num9 = SDL_SCANCODE_9,

        F1 = SDL_SCANCODE_F1,
        F2 = SDL_SCANCODE_F2,
        F3 = SDL_SCANCODE_F3,
        F4 = SDL_SCANCODE_F4,
        F5 = SDL_SCANCODE_F5,
        F6 = SDL_SCANCODE_F6,
        F7 = SDL_SCANCODE_F7,
        F8 = SDL_SCANCODE_F8,
        F9 = SDL_SCANCODE_F9,
        F10 = SDL_SCANCODE_F10,
        F11 = SDL_SCANCODE_F11,
        F12 = SDL_SCANCODE_F12,

        Numpad0 = SDL_SCANCODE_KP_0,
        Numpad1 = SDL_SCANCODE_KP_1,
        Numpad2 = SDL_SCANCODE_KP_2,
        Numpad3 = SDL_SCANCODE_KP_3,
        Numpad4 = SDL_SCANCODE_KP_4,
        Numpad5 = SDL_SCANCODE_KP_5,
        Numpad6 = SDL_SCANCODE_KP_6,
        Numpad7 = SDL_SCANCODE_KP_7,
        Numpad8 = SDL_SCANCODE_KP_8,
        Numpad9 = SDL_SCANCODE_KP_9,

        NumpadPlus = SDL_SCANCODE_KP_PLUS,
        NumpadMinus = SDL_SCANCODE_KP_MINUS,
        NumpadMultiply = SDL_SCANCODE_KP_MULTIPLY,
        NumpadDivide = SDL_SCANCODE_KP_DIVIDE,
        NumpadDecimal = SDL_SCANCODE_KP_DECIMAL,

        NumpadEnter = SDL_SCANCODE_KP_ENTER,

        Esc = SDL_SCANCODE_ESCAPE,
        Enter = SDL_SCANCODE_RETURN,
        Space = SDL_SCANCODE_SPACE,
        BackSpace = SDL_SCANCODE_BACKSPACE,
        Tab = SDL_SCANCODE_TAB,
        Menu = SDL_SCANCODE_MENU,
        CapsLock = SDL_SCANCODE_CAPSLOCK,
        NumLock = SDL_SCANCODE_NUMLOCKCLEAR,
        ScrollLock = SDL_SCANCODE_SCROLLLOCK,
        LeftCtrl = SDL_SCANCODE_LCTRL,
        RightCtrl = SDL_SCANCODE_RCTRL,
        LeftShift = SDL_SCANCODE_LSHIFT,
        RightShift = SDL_SCANCODE_RSHIFT,
        LeftAlt = SDL_SCANCODE_LALT,
        RightAlt = SDL_SCANCODE_RALT,
        LeftWindows = SDL_SCANCODE_LGUI,
        RightWindows = SDL_SCANCODE_RGUI,

        LeftBracket = SDL_SCANCODE_LEFTBRACKET,
        RightBracket = SDL_SCANCODE_RIGHTBRACKET,

        Dash = SDL_SCANCODE_MINUS,
        Equals = SDL_SCANCODE_EQUALS,

        Slash = SDL_SCANCODE_SLASH,
        BackSlash = SDL_SCANCODE_BACKSLASH,
        Peroid = SDL_SCANCODE_PERIOD,
        Comma = SDL_SCANCODE_COMMA,
        Semicolon = SDL_SCANCODE_SEMICOLON,
        Apostrophe = SDL_SCANCODE_APOSTROPHE,
        Grave = SDL_SCANCODE_GRAVE,

        Print = SDL_SCANCODE_PRINTSCREEN,
        Pause = SDL_SCANCODE_PAUSE,

        Insert = SDL_SCANCODE_INSERT,
        Delete = SDL_SCANCODE_DELETE,
        Home = SDL_SCANCODE_HOME,
        End = SDL_SCANCODE_END,
        PageUp = SDL_SCANCODE_PAGEUP,
        PageDown = SDL_SCANCODE_PAGEDOWN,

        MouseLeft = SDL_NUM_SCANCODES - 5,
        MouseRight = SDL_NUM_SCANCODES - 4,
        MouseMiddle = SDL_NUM_SCANCODES - 3,
        MouseButton1 = SDL_NUM_SCANCODES - 2,
        MouseButton2 = SDL_NUM_SCANCODES - 1
    };

    constexpr auto NUM_MIN_MOUSE = static_cast<int>(Key::MouseLeft);
    constexpr auto NUM_KEY = SDL_NUM_SCANCODES;

    class InputManager final
    {
    public:
        InputManager();

        InputManager(const InputManager&) = delete;
        InputManager(InputManager&&) = delete;

        InputManager& operator=(const InputManager&) = delete;
        InputManager& operator=(InputManager&&) = delete;

        ~InputManager() = default;

        void Update();

        inline bool IsRelativeMove() const noexcept
        {
            return isRelative;
        }

        void SetRelativeMouseMode(bool value);

        inline bool IsButtonDown(Key key) const noexcept
        {
            return IsCurDown(key);
        }

        inline bool IsButtonUp(Key key) const noexcept
        {
            return !IsCurDown(key);
        }

        inline bool IsButtonPress(Key key) const noexcept
        {
            return IsCurDown(key) && !IsOldDown(key);
        }

        inline bool IsButtonRelease(Key key) const noexcept
        {
            return !IsCurDown(key) && IsOldDown(key);
        }

        inline Math::IntVector2 GetMousePos() const noexcept
        {
            return mousePos;
        }

        inline int32_t GetWheelMove() const noexcept
        {
            return wheelMove;
        }

    private:
        bool IsCurDown(Key key) const noexcept;
        bool IsOldDown(Key key) const noexcept;

    private:
        const uint8_t* curKeyState;
        uint8_t oldKeyState[SDL_NUM_SCANCODES];

        uint32_t curButtonState;
        uint32_t oldButtonState;

        Math::IntVector2 mousePos;
        int32_t wheelMove;

        bool isRelative;
    };
}