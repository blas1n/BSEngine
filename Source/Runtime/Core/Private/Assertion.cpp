#include "Assertion.h"
#include "Log.h"

DEFINE_LOG_CATEGORY(LogAssert)

#ifndef NDEBUG
	
void LogToFail(bool isCritical, const char* expr, const char* file, int32 line, const std::string& msg)
{
	Log(LogAssert, isCritical ? LogVerbosity::Critical : LogVerbosity::Error,
		"{} failed: {} {}, file: {}, line: {}", isCritical ? "Check" : "Ensure", expr, msg, file, line);
}

#endif