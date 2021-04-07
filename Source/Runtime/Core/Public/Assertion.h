#pragma once

#include "LogCategory.h"
#include "Platform.h"
#include <fmt/core.h>

CORE_API DECLARE_LOG_CATEGORY(LogAssert)

#ifdef NDEBUG

#	define CheckMsg(expr, fmt, ...) (void)0
#	define Check(expr) (void)0

#	define EnsureMsg(expr, fmt, ...) !!(expr)
#	define Ensure(expr) !!(expr)

#else

CORE_API void LogToFail(bool isCritical, const char* expr, const char* file, int32 line, const std::string& msg);

template <class Str, class... Args>
void LogToFail(bool isCritical, const char* expr, const char* file, int32 line, const Str& format, Args... args)
{
	LogToFail(isCritical, expr, file, line, fmt::format(format, args...));
}

#	ifdef _WIN32
#		define DEBUG_BREAK() (void)(IsDebugging() && (::__debugbreak(), true))
#	else
#		include <csignal>
#		define DEBUG_BREAK() (void)(IsDebugging() && (::std::raise(SIGTRAP), true))
#	endif

#	define CheckMsg(expr, fmt, ...) (void)(!!(expr) || (LogToFail(true, #expr, __FILE__, __LINE__, fmt, ##__VA_ARGS__), DEBUG_BREAK(), false))
#	define Check(expr) CheckMsg(expr, "")

#	define EnsureMsg(expr, fmt, ...) (!!(expr) || (LogToFail(false, #expr, __FILE__, __LINE__, fmt, ##__VA_ARGS__), DEBUG_BREAK(), false))
#	define Ensure(expr) EnsureMsg(exprt, "")

#endif
