#include "Assertion.h"
#include "Log.h"

DEFINE_LOG_CATEGORY(LogAssert)

#ifndef NDEBUG
	
void Impl::LogToFail(bool isCritical, const Char* expr, const Char* file, BSBase::int32 line, const String& msg) noexcept
{
	Log(LogAssert, isCritical ? LogVerbosity::Critical : LogVerbosity::Error,
		"{} failed: {} {}, file: {}, line: {}", isCritical ? "Check" : "Ensure", expr, msg, file, line);
}

#endif