#pragma once

#include <utility>

namespace Util
{
    template <class T, int N>
    constexpr int ArraySize(const T(&arr)[N]) noexcept
    {
        return N;
    }

    template <class T>
    inline void DeleteObjects(T&& ptr)
    {
        if (ptr)
        {
            delete ptr;
            ptr = nullptr;
        }
    }

    template <class T, class... Others>
    void DeleteObjects(T&& ptr, Others&&... others)
    {
        DeleteObjects(std::forward<T>(ptr));
        DeleteObjects(std::forward<Others>(others)...);
    }
}