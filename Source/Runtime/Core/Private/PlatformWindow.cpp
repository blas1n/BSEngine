#ifdef _WIN32

#define NOMINMAX
#define WIN32_LEAN_AND_MEAN

#include "Platform.h"
#include <Windows.h>
#include "fmt/core.h"
#include "Assertion.h"

#ifndef NDEBUG

bool Impl::IsDebuggingImpl() noexcept
{
    return IsDebuggerPresent();
}

#endif

namespace
{
    String GetLastErrorMsg()
    {
        constexpr static auto MaxSize = 512;
        static wchar_t buffer[MaxSize];

        FormatMessageW(
            FORMAT_MESSAGE_FROM_SYSTEM,
            nullptr,
            GetLastError(),
            0,
            buffer,
            MaxSize,
            nullptr
        );

        return String{ buffer };
    }
}

Dll::Dll(const String& inPath)
    : path(inPath)
{
    dll = LoadLibrary(path.c_str());
    Check(dll, u"{}: cannot load module, {}", path, GetLastErrorMsg());
}

Dll::Dll(const Dll& other)
    : path(other.path)
{
    if (dll) FreeLibrary(reinterpret_cast<HMODULE>(dll));
    dll = other.dll;
}

Dll::Dll(Dll&& other) noexcept
    : path(std::move(other.path))
{
    if (dll) FreeLibrary(reinterpret_cast<HMODULE>(dll));
    dll = std::move(other.dll);

}

Dll& Dll::operator=(const Dll& other)
{
    if (dll) FreeLibrary(reinterpret_cast<HMODULE>(dll));
    dll = other.dll;
    path = other.path;
}

Dll& Dll::operator=(Dll&& other) noexcept
{
    if (dll) FreeLibrary(reinterpret_cast<HMODULE>(dll));
    dll = std::move(other.dll);
    path = std::move(other.path);
}

Dll::~Dll()
{
    FreeLibrary(reinterpret_cast<HMODULE>(dll));
}

void* Dll::GetSymbol(const String& name) const
{
    auto* symbol = FindSymbol(name);
    if (!symbol)
    {
        fmt::format("");
        const auto msg = fmt::format(u"Path: {}, Name: {}, {}", path, name, GetLastErrorMsg());
        throw std::runtime_error{ msg };
    }
    return symbol;
}

void* Dll::FindSymbol(const String& name) const noexcept
{
    return reinterpret_cast<void*>(GetProcAddress(reinterpret_cast<HMODULE>(dll), name.c_str()));
}

#endif
