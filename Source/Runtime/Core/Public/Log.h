#pragma once

#include "Core.h"
#include <fmt/core.h>
#include "LogCategory.h"

enum class LogVerbosity : uint8
{
	Debug, Log, Display, Warn, Error, Critical
};

CORE_API void Log(const LogCategory& category, LogVerbosity verbosity, const std::string& msg);

template <class Str, class... Args>
void Log(const LogCategory& category, LogVerbosity verbocity, const Str& format, const Args&... args)
{
	Log(category, level, fmt::format(format, args...));
}