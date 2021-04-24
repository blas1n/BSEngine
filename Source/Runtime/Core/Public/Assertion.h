#pragma once

#include "BSBase/Type.h"
#include "fmt/format.h"
#include "CharSet.h"
#include "Platform.h"

#ifdef NDEBUG

#	define Assert(expr)
#	define AssertMsg(expr, fmt, ...)

#	define Ensure(expr)
#	define EnsureMsg(expr, fmt, ...)

#else

namespace Impl
{
	CORE_API void LogToFail(bool isCritical, const Char* expr, const Char* file, BSBase::int32 line, const String& msg) noexcept;

	template <class Str, class... Args>
	void LogToFail(bool isCritical, const Char* expr, const Char* file, BSBase::int32 line, const Str& format, Args... args) noexcept
	{
		LogToFail(isCritical, expr, file, line, fmt::format(format, args...));
	}
}

#		define DEBUG_BREAK() (void)(::IsDebugging() && (::__debugbreak(), true))

//#	ifdef WINDOWS
//#		define DEBUG_BREAK() (void)(::IsDebugging() && (::__debugbreak(), true))
//#	else
//#		include <csignal>
//#		define DEBUG_BREAK() (void)(::IsDebugging() && (::std::raise(SIGTRAP), true))
//#	endif

#	define AssertMsg(expr, fmt, ...) (void)(!!(expr) || (Impl::LogToFail(true, \
		u#expr, ADD_PREFIX(u, __FILE__), __LINE__, fmt, ##__VA_ARGS__), DEBUG_BREAK(), false))

#	define Assert(expr) AssertMsg(expr, STR(""))

#	define EnsureMsg(expr, fmt, ...) (!!(expr) || (Impl::LogToFail(false, \
		u#expr, ADD_PREFIX(u, __FILE__), __LINE__, fmt, ##__VA_ARGS__), DEBUG_BREAK(), false))

#	define Ensure(expr) EnsureMsg(expr, STR(""))

#endif
