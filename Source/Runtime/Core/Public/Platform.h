#pragma once

#include "Core.h"
#include "CharSet.h"

// The warning is unnecessary because the path variable is used internally.
#pragma warning(disable: 4251)

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
    return Impl::IsDebuggingImpl();
#endif
}

class CORE_API Dll final
{
public:
    explicit Dll(const String& inPath);

    Dll(const Dll& other);
    Dll(Dll&& other) noexcept;

    Dll& operator=(const Dll& other);
    Dll& operator=(Dll&& other) noexcept;

    ~Dll();

    [[nodiscard]] void* GetSymbol(const String& name) const;
    [[nodiscard]] void* FindSymbol(const String& name) const noexcept;

    template <class T>
    [[nodiscard]] T& GetSymbol(const String& name) const
    {
        return *reinterpret_cast<T*>(GetSymbol(name));
    }

    template <class T>
    [[nodiscard]] T* FindSymbol(const String& name) const noexcept
    {
        return reinterpret_cast<T*>(FindSymbol(name));
    }

    template <class Fn, class... Args>
    decltype(auto) Call(const String& name, Args&&... args) const
    {
        return GetSymbol<Fn>(name)(std::forward<Args>(args)...);
    }

private:
    void* dll;
    String path;
};
