#include "Logger.h"
#include "spdlog/spdlog.h"

namespace
{
    [[nodiscard]] constexpr spdlog::level::level_enum ToSpdLogLevel(LogVerbosity verbosity) noexcept
    {
        constexpr spdlog::level::level_enum mapper[]{ spdlog::level::debug, spdlog::level::info,
            spdlog::level::info, spdlog::level::warn, spdlog::level::err, spdlog::level::critical };

        return mapper[static_cast<BSBase::uint8>(verbosity)];
    }
}

Logger::Logger(Name name)
{
    auto str = CastCharSet<char>(StringView{ name.ToString() });
    if (const auto logger = spdlog::get(str))
    {
        impl = logger;
        return;
    }

    impl = std::make_shared<spdlog::logger>(std::move(str));
    spdlog::details::registry::instance().initialize_logger(impl);
    impl->set_level(spdlog::level::debug);
    impl->flush_on(spdlog::level::critical);
}

Logger::~Logger()
{
    impl->flush();
}

void Logger::RemoveSink(size_t index)
{
    impl->sinks().erase(impl->sinks().cbegin() + index);
}

void Logger::LogImpl(LogVerbosity verbosity, String message)
{
    impl->log(ToSpdLogLevel(verbosity),
        CastCharSet<char>(StringView{ std::move(message) }));
}

size_t Logger::AddSinkImpl(std::shared_ptr<spdlog::sinks::sink> sink)
{
    impl->sinks().emplace_back(std::move(sink));
    return impl->sinks().size() - 1;
}
