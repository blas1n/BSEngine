#pragma once

#include "BSBase/Type.h"
#include "fmt/format.h"
#include "CharSet.h"

enum class LogVerbosity : BSBase::uint8
{
	Debug, Log, Display, Warn, Error, Critical
};

namespace Impl
{
	CORE_API void Log(StringView category, LogVerbosity verbosity, const String& msg);
}

template <class... Args>
void Log(StringView category, LogVerbosity verbosity, const String& format, Args&&... args)
{
	Impl::Log(category, verbosity, fmt::format(format, std::forward<Args>(args)...));
}
