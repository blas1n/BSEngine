#include "Assertion.h"

#ifndef NDEBUG

#include "Logger.h"

void Impl::LogToFail(bool isCritical, const Char* expr, const Char* file, BSBase::int32 line, const String& msg) noexcept
{
	static Logger assertLogger{ ReservedName::Assert };
	static Logger ensureLogger{ ReservedName::Ensure };
	static bool bInit = false;

	if (!bInit)
	{
		assertLogger.AddSink<Sink::FileSink>(STR("Assert.log"));
		ensureLogger.AddSink<Sink::FileSink>(STR("Ensure.log"));

		assertLogger.AddSink<Sink::StderrSink>();
		ensureLogger.AddSink<Sink::StderrSink>();
		
		bInit = true;
	}

	if (isCritical)
	{
		assertLogger.Log(LogVerbosity::Critical,
			STR("{} failed: {} {}, file: {}, line: {}"),
			isCritical ? STR("Assert") : STR("Ensure"), expr, msg, file, line
		);
	}
	else
	{
		ensureLogger.Log(LogVerbosity::Error,
			STR("{} failed: {} {}, file: {}, line: {}"),
			isCritical ? STR("Assert") : STR("Ensure"), expr, msg, file, line
		);
	}
}

#endif
