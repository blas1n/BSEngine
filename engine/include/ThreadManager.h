#pragma once

#include "ManagerMacro.h"
#include "Array.h"
#include "Queue.h"
#include <functional>
#include <future>
#include <mutex>

namespace BE
{
	class ThreadManager
	{
	public:
		ThreadManager() noexcept;

		void Init() noexcept;
		void Release() noexcept;

		template <class Fn, class... Args>
		std::future<std::invoke_result_t<Fn, Args...>> AddTask(
			Fn&& fn, Args&&... args);

	private:
		void ThreadWork();

	private:
		Array<std::thread> threads;
		Queue<std::function<void()>> tasks;
		std::condition_variable cv;
		std::mutex taskMutex;
		bool isEnd;
	};

	template <class Fn, class... Args>
	std::future<std::invoke_result_t<Fn, Args...>> ThreadManager::AddTask(Fn&& fn, Args&&... args) {
		auto task = std::make_shared<
			std::packaged_task<std::invoke_result_t<Fn, Args...>()>>(
				std::bind(std::forward<Fn>(fn), std::forward<Args>(args)...)
				);

		taskMutex.lock();
		tasks.push([task] { (*task)(); });
		taskMutex.unlock();

		cv.notify_one();
		return task->get_future();
	}

	CREATE_MANAGER_ACCESSER(ThreadManager)
}