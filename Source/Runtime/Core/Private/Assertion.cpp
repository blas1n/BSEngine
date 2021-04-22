#include "Assertion.h"
#include "Log.h"

#ifndef NDEBUG

void Impl::LogToFail(bool isCritical, const Char* expr, const Char* file, BSBase::int32 line, const String& msg) noexcept
{
	Log(STR(""), isCritical ? LogVerbosity::Critical : LogVerbosity::Error,
		STR("{} failed: {} {}, file: {}, line: {}"),
		isCritical ? STR("Check") : STR("Ensure"), expr, msg, file, line);
}

#endif
