#pragma once

#include <filesystem>
#include <mutex>
#include "BSBase/Type.h"
#include "fmt/format.h"
#include "spdlog/sinks/daily_file_sink.h"
#include "spdlog/sinks/null_sink.h"
#include "spdlog/sinks/stdout_color_sinks.h"
#include "CharSet.h"

namespace spdlog
{
    class logger;
}

namespace Sink
{
    using NullSink = spdlog::sinks::null_sink_mt;
    
    using StdoutSink = spdlog::sinks::stdout_color_sink_mt;
    using StderrSink = spdlog::sinks::stderr_color_sink_mt;

    class CORE_API FileSink final : public spdlog::sinks::base_sink<std::mutex>
    {
        using FileNameCalc = spdlog::sinks::daily_filename_calculator;

    public:
        FileSink(StringView inName, BSBase::uint8 inHour = 0u,
            BSBase::uint8 inMinute = 0u, bool inTruncate = false, BSBase::uint16 inMaxFile = 0u)
            : maxFile(inMaxFile),
            hour(inHour),
            minute(inMinute),
            truncate(inTruncate)
        {
            name = std::filesystem::current_path().parent_path() / "Saved" / "Logs" / inName;
            create_directories(name);

            const auto now = spdlog::log_clock::now();
            const auto curFilename = FileNameCalc::calc_filename(name.string(), GetNow(now));
            
            fileHelper.open(curFilename, truncate);
            timePoint = GetNextTimePoint();

            if (maxFile > 0)
                InitFileNames();
        }

        [[nodiscard]] String GetName() const noexcept
        {
            return name.u16string();
        }

    private:
        void sink_it_(const spdlog::details::log_msg& msg) override
        {
            const auto time = msg.time;
            bool should_rotate = time >= timePoint;

            if (should_rotate)
            {
                auto filename = FileNameCalc::calc_filename(name.string(), GetNow(time));
                fileHelper.open(filename, truncate);
                timePoint = GetNextTimePoint();
            }
            
            spdlog::memory_buf_t formatted;
            base_sink<std::mutex>::formatter_->format(msg, formatted);
            fileHelper.write(formatted);

            if (should_rotate && maxFile > 0)
                DeleteOld();
        }

        void flush_() override
        {
            fileHelper.flush();
        }

        void InitFileNames()
        {
            using spdlog::details::os::path_exists;

            filenames = decltype(filenames)(static_cast<size_t>(maxFile));
            std::vector<spdlog::filename_t> names;
            auto now = spdlog::log_clock::now();

            while (filenames.size() < maxFile)
            {
                auto filename = FileNameCalc::calc_filename(name.string(), GetNow(now));
                if (!path_exists(filename))
                    break;

                names.emplace_back(filename);
                now -= std::chrono::hours(24);
            }

            for (auto iter = names.rbegin(); iter != names.rend(); ++iter)
                filenames.push_back(std::move(*iter));
        }

        [[nodiscard]] tm GetNow(spdlog::log_clock::time_point tp)
        {
            const auto now = spdlog::log_clock::to_time_t(tp);
            return spdlog::details::os::localtime(now);
        }

        [[nodiscard]] spdlog::log_clock::time_point GetNextTimePoint()
        {
            const auto now = spdlog::log_clock::now();

            auto date = GetNow(now);
            date.tm_hour = hour;
            date.tm_min = minute;
            date.tm_sec = 0;

            const auto time = spdlog::log_clock::from_time_t(std::mktime(&date));
            return (time > now) ? time : (time + std::chrono::hours(24));
        }

        void DeleteOld()
        {
            using spdlog::details::os::filename_to_str;
            using spdlog::details::os::remove_if_exists;

            auto crtFile = fileHelper.filename();
            if (filenames.full())
            {
                const auto oldName = std::move(filenames.front());
                filenames.pop_front();

                if (remove_if_exists(oldName.string()))
                    filenames.push_back(std::move(crtFile));
            }

            filenames.push_back(std::move(crtFile));
        }

    private:
        std::filesystem::path name;
        spdlog::details::circular_q<std::filesystem::path> filenames;
        spdlog::log_clock::time_point timePoint;
        spdlog::details::file_helper fileHelper;
            
        BSBase::uint16 maxFile;
        BSBase::uint8 hour;
        BSBase::uint8 minute;
        BSBase::uint8 truncate : 1;
    };
}

enum class LogVerbosity : BSBase::uint8
{
	Debug, Log, Display, Warn, Error, Critical
};

class CORE_API Logger final
{
public:
    Logger(StringView name);

    Logger(const Logger&) = default;
    Logger(Logger&&) noexcept = default;
    
    Logger& operator=(const Logger&) = default;
    Logger& operator=(Logger&&) noexcept = default;

    ~Logger();

    template <class... Args>
    void Log(LogVerbosity verbosity, const String& format, Args&&... args)
    {
        LogImpl(verbosity, fmt::format(format, std::forward<Args>(args)...));
    }

    template <class Sink, class... Args>
    size_t AddSink(Args&&... args)
    {
        return AddSinkImpl(std::make_shared<Sink>(std::forward<Args>(args)...));
    }

    void RemoveSink(size_t index);

private:
    void LogImpl(LogVerbosity verbosity, String message);
    size_t AddSinkImpl(std::shared_ptr<spdlog::sinks::sink> sink);

private:
    std::shared_ptr<spdlog::logger> impl;
};
