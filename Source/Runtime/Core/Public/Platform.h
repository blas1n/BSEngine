#pragma once

#include "Core.h"

#ifndef NDEBUG
namespace Detail
{
    [[nodiscard]] CORE_API bool IsDebuggingImpl() noexcept;
}
#endif

[[nodiscard]] NO_ODR bool IsDebugging() noexcept
{
#ifdef NDEBUG
    return false;
#else
    return Detail::IsDebuggingImpl();
#endif
}