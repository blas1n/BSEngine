#pragma once

#include "BSBase/Type.h"
#include "fmt/format.h"
#include "CharSet.h"
#include "Platform.h"

#ifdef NDEBUG

#	define Assert(expr)
#	define AssertLog(expr, logger)
#	define AssertMsg(expr, logger, fmt, ...)

#	define Ensure(expr)
#	define EnsureLog(expr, logger)
#	define EnsureMsg(expr, logger, fmt, ...)

#else

class Logger;

namespace Impl
{
	CORE_API void LogToFail(Logger& logger, bool isCritical, const Char* expr, const Char* file, BSBase::int32 line, const String& msg) noexcept;

	template <class Str, class... Args>
	void LogToFail(Logger& logger, bool isCritical, const Char* expr, const Char* file, BSBase::int32 line, const Str& format, Args... args) noexcept
	{
		LogToFail(logger, isCritical, expr, file, line, fmt::format(format, args...));
	}
}

#	ifdef PLATFORM_WINDOWS
#		define DEBUG_BREAK() (void)(IsDebugging() && (::__debugbreak(), true))
#	else
#		include <csignal>
#		define DEBUG_BREAK() (void)(IsDebugging() && (::std::raise(SIGTRAP), true))
#	endif

#	define Assert(expr) (void)(!!(expr) || (DEBUG_BREAK(), false))

#	define AssertMsg(expr, logger, fmt, ...) (void)(!!(expr) || (Impl::LogToFail(true, \
		u#expr, ADD_PREFIX(u, __FILE__), __LINE__, fmt, ##__VA_ARGS__), DEBUG_BREAK(), false))

#	define AssertLog(expr, logger) AssertMsg(expr, logger, STR(""))

#	define Ensure(expr) (!!(expr) || (DEBUG_BREAK(), false))

#	define EnsureMsg(expr, logger, fmt, ...) (!!(expr) || (Impl::LogToFail(false, \
		u#expr, ADD_PREFIX(u, __FILE__), __LINE__, fmt, ##__VA_ARGS__), DEBUG_BREAK(), false))

#	define EnsureLog(expr, logger) EnsureMsg(expr, logger, STR(""))

#endif
