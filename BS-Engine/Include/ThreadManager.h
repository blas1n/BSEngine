#pragma once

#include "IManager.h"
#include "Array.h"
#include "Queue.h"
#include <thread>
#include <future>
#include <functional>
#include <mutex>
#include <condition_variable>
#include <cstdio>

class BS_API ThreadManager final : public IManager
{
public:
	bool Init() noexcept override;
	void Update(float deltaTime) noexcept override {}
	void Release() noexcept override;

	template <class Fn, class... Args> 
	std::future<std::invoke_result_t<Fn, Args...>> AddTask(Fn&& fn, Args&&... args);

private:
	void Work() noexcept;

private:
	Array<std::thread> threads;
	Queue<std::function<void()>> tasks;
	std::condition_variable cv;
	std::mutex jobMutex;
	bool isEnd = false;
};

template <class Fn, class... Args>
std::future<std::invoke_result_t<Fn, Args...>> ThreadManager::AddTask(Fn&& fn, Args&&... args)
{
	auto task = std::make_shared<
		std::packaged_task<std::invoke_result_t<Fn, Args...>()>>(
			std::bind(std::forward<Fn>(fn), std::forward<Args>(args)...)
		);

	auto job_result_future = task->get_future();

	jobMutex.lock();
	tasks.push([task] { (*task)(); });
	jobMutex.unlock();

	cv.notify_one();
	return job_result_future;
}