#pragma once

#include "BSBase/Type.h"
#include "fmt/format.h"
#include "CharSet.h"
#include "LogCategory.h"
#include "Platform.h"

CORE_API DECLARE_LOG_CATEGORY(LogAssert)

#ifdef NDEBUG

#	define CheckMsg(expr, fmt, ...) (void)expr
#	define Check(expr) (void)expr

#	define EnsureMsg(expr, fmt, ...) !!(expr)
#	define Ensure(expr) !!(expr)

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

#	ifdef _WIN32
#		define DEBUG_BREAK() (void)(IsDebugging() && (::__debugbreak(), true))
#	else
#		include <csignal>
#		define DEBUG_BREAK() (void)(IsDebugging() && (::std::raise(SIGTRAP), true))
#	endif

#	define CheckMsg(expr, fmt, ...) (void)(!!(expr) || (Impl::LogToFail(true, \
		u#expr, ADD_PREFIX(u, __FILE__), __LINE__, fmt, ##__VA_ARGS__), DEBUG_BREAK(), false))

#	define Check(expr) CheckMsg(expr, u"")

#	define EnsureMsg(expr, fmt, ...) (!!(expr) || (Impl::LogToFail(false, \
		u#expr, ADD_PREFIX(u, __FILE__), __LINE__, fmt, ##__VA_ARGS__), DEBUG_BREAK(), false))

#	define Ensure(expr) EnsureMsg(exprt, u"")

#endif
