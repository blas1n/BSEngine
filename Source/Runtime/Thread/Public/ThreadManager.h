#pragma once

#include "Core.h"
#include <vector>
#include <queue>
#include <functional>
#include <future>
#include <mutex>

class ThreadManager final
{
public:
	void Init() noexcept;
	void Release() noexcept;

	template <class Fn, class... Args>
	decltype(auto) AddTask(Fn&& fn, Args&&... args) noexcept;

private:
	void ThreadWork() noexcept;

private:
	std::vector<std::thread> threads;
	std::queue<std::function<void()>> tasks;
	std::condition_variable cv;
	std::mutex taskMutex;
	bool isEnd;
};

template <class Fn, class... Args>
decltype(auto) ThreadManager::AddTask(Fn&& fn, Args&&... args) noexcept
{
	auto task = std::make_shared
		<std::packaged_task<std::invoke_result_t<Fn, Args...>()>>(
			std::bind(Forward<Fn>(fn), Forward<Args>(args)...)
		);

	taskMutex.lock();
	tasks.push([task] { (*task)(); });
	taskMutex.unlock();

	cv.notify_one();
	return task->get_future();
}