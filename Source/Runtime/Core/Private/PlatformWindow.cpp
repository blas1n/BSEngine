#ifdef _WIN32

#define NOMINMAX
#define WIN32_LEAN_AND_MEAN

#include "Platform.h"
#include "Assertion.h"
#include <Windows.h>

#ifndef NDEBUG

namespace Detail
{
    bool IsDebuggingImpl() noexcept
    {
        return IsDebuggerPresent();
    }
}

#endif

namespace
{
    std::string GetLastErrorMsg()
    {
        constexpr static auto MaxSize = 512;
        static char buffer[MaxSize];

        FormatMessage(
            FORMAT_MESSAGE_FROM_SYSTEM,
            nullptr,
            GetLastError(),
            0,
            buffer,
            MaxSize,
            nullptr
        );

        return std::string{ buffer };
    }
}

Dll::Dll(const std::string& inPath)
    : path(inPath)
{
    dll = LoadLibrary(path.c_str());
    Check(dll, "{}: cannot load module: {}", filepath, GetLastErrorMsg());
}

Dll::~Dll()
{
    FreeLibrary(reinterpret_cast<HMODULE>(dll));
}

void* Dll::FindSymbol(const std::string& name) const noexcept
{
    return reinterpret_cast<void*>(GetProcAddress(reinterpret_cast<HMODULE>(dll), name.c_str()));
}

#endif