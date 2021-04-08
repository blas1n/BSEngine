#ifdef _WIN32

#define NOMINMAX
#define WIN32_LEAN_AND_MEAN

#include "Platform.h"
#include <locale>
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
        
        const std::wstring msg(buffer);
        return String(msg.begin(), msg.end());
    }
}

Dll::Dll(const String& inPath)
    : path(inPath)
{
    const std::wstring wPath(path.cbegin(), path.cend());
    dll = LoadLibraryW(wPath.c_str());
    CheckMsg(dll, u"{}: cannot load module, {}", path, GetLastErrorMsg());
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
    const auto symbol = FindSymbol(name);
    if (EnsureMsg(symbol, u"Path: {}, Name: {}, {}", path, name, GetLastErrorMsg()))
        return symbol;

    return nullptr;
}

void* Dll::FindSymbol(const String& name) const noexcept
{
    const auto& convert = std::use_facet<std::codecvt<Char, char, std::mbstate_t>>(std::locale());
    std::mbstate_t state;
    std::string buf((name.size() + 1) * convert.max_length(), 0);
    
    const Char* in = name.c_str();
    char* out = buf.data();

    convert.out(state, name.c_str(), name.c_str() + name.size(), in, buf.data(), buf.data() + buf.size(), out);
    return reinterpret_cast<void*>(GetProcAddress(reinterpret_cast<HMODULE>(dll), buf.c_str()));
}

#endif
