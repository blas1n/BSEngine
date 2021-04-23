#include "Assertion.h"
#include "Logger.h"

#ifndef NDEBUG

void Impl::LogToFail(Logger& logger, bool isCritical, const Char* expr, const Char* file, BSBase::int32 line, const String& msg) noexcept
{
	logger.Log(
		isCritical ? LogVerbosity::Critical : LogVerbosity::Error,
		STR("{} failed: {} {}, file: {}, line: {}"),
		isCritical ? STR("Assert") : STR("Ensure"), expr, msg, file, line
	);
}

#endif
