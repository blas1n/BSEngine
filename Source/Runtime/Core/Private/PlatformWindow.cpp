#ifdef _WIN32

#define NOMINMAX
#define WIN32_LEAN_AND_MEAN

#include "Platform.h"
#include <Windows.h>

#ifndef NDEBUG
bool Detail::IsDebuggingImpl() noexcept
{
    return IsDebuggerPresent();
}
#endif

#endif