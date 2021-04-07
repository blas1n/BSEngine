#pragma once

#include "Core.h"
#include "CharSet.h"

#ifndef NDEBUG
namespace Impl
{
    [[nodiscard]] CORE_API bool IsDebuggingImpl() noexcept;
}
#endif

[[nodiscard]] NO_ODR bool IsDebugging() noexcept
{
#ifdef NDEBUG
    return false;
#else
    return ::Detail::IsDebuggingImpl();
#endif
}

class CORE_API Dll final
{
public:
    explicit Dll(const std::string& inPath);

    Dll(const Dll& other);
    Dll(Dll&& other) noexcept;

    Dll& operator=(const Dll& other);
    Dll& operator=(Dll&& other) noexcept;

    ~Dll();

    [[nodiscard]] void* GetSymbol(const std::string& name) const;
    [[nodiscard]] void* FindSymbol(const std::string& name) const noexcept;

    template <class T>
    [[nodiscard]] T& GetSymbol(const std::string& name) const
    {
        return *(T*)GetSymbol(name);
    }

    template <class T>
    [[nodiscard]] T* FindSymbol(const std::string& name) const noexcept
    {
        return (T*)FindSymbol(name);
    }

    template <class Fn, class... Args>
    decltype(auto) Call(const std::string& name, Args&&... args) const
    {
        return GetSymbol<Fn>(name)(std::forward<Args>(args)...);
    }

private:
    void* dll;
    std::string path;
};