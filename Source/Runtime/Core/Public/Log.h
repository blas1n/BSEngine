#pragma once

#include "BSBase/Type.h"
#include "fmt/core.h"
#include "CharSet.h"
#include "LogCategory.h"

enum class LogVerbosity : BSBase::uint8
{
	Debug, Log, Display, Warn, Error, Critical
};

namespace Impl
{
	CORE_API void Log(const LogCategory& category, LogVerbosity verbosity, const String& msg);
}

template <class... Args>
void Log(const LogCategory& category, LogVerbosity verbosity, const String& format, Args&&... args)
{
	Impl::Log(category, verbosity, fmt::format(format, std::forward<Args>(args)...));
}
