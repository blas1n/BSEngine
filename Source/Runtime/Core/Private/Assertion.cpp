#include "Assertion.h"
#include "Log.h"

#ifndef NDEBUG

DEFINE_LOG_CATEGORY(LogAssert)

void Impl::LogToFail(bool isCritical, const Char* expr, const Char* file, BSBase::int32 line, const String& msg) noexcept
{
	Log(LogAssert, isCritical ? LogVerbosity::Critical : LogVerbosity::Error,
		STR("{} failed: {} {}, file: {}, line: {}"),
		isCritical ? STR("Check") : STR("Ensure"), expr, msg, file, line);
}

#endif
