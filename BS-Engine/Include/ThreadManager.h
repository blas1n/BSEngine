#pragma once

#include "IManager.h"
#include <vector>
#include <future>
#include <queue>
#include <mutex>

class ThreadManager : public IManager {
public:
	bool Init() noexcept override;
	void Update(float deltaTime) noexcept override {}
	void Release() noexcept override;

	template <class Fn, class... Args>
	std::future<std::invoke_result_t<Fn, Args...>> AddTask(
		Fn&& fn, Args&&... args);

private:
	void ThreadWork();

private:
	std::vector<std::thread> threads;
	std::queue<std::function<void()>> tasks;
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