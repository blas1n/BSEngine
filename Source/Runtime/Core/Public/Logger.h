#pragma once

#include "BSBase/Type.h"
#include "fmt/format.h"
#include "CharSet.h"

namespace spdlog { class logger; }

enum class LogVerbosity : BSBase::uint8
{
	Debug, Log, Display, Warn, Error, Critical
};

struct Console {};

class CORE_API Logger final
{
public:
    Logger(StringView name, StringView dir);
    Logger(StringView name, const Console&);

    Logger(const Logger&) = default;
    Logger& operator=(Logger&&) noexcept = default;
    
    Logger(const Logger&) = default;
    Logger& operator=(Logger&&) noexcept = default;

    ~Logger();

    template <class... Args>
    void Log(LogVerbosity verbosity, const String& format, Args&&... args)
    {
        Log(verbosity, fmt::format(format, std::forward<Args>(args)...));
    }

private:
    void Log(LogVerbosity verbosity, const String& message);

private:
    std::shared_ptr<spdlog::logger> impl;
};
