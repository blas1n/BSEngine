#pragma once

#include "Core.h"
#include "Manager.h"
#include <vector>
#include <queue>
#include <functional>
#include <future>
#include <mutex>
#include "Delegate.h"

class THREAD_API ThreadManager final : public Manager
{
public:
	[[nodiscard]] bool Init() noexcept override;
	void Release() noexcept override;

	template <class R, class... Args>
	decltype(auto) AddTask(const Delegate<R(Args...)>& task, Args&&... args) noexcept;

	template <class R, class... Args>
	decltype(auto) AddTask(Delegate<R(Args...)>&& task, Args&&... args) noexcept;

	[[nodiscard]] bool IsMainThread() const noexcept { return mainThreadId == std::this_thread::get_id(); }

private:
	void ThreadWork() noexcept;

	template <class R>
	decltype(auto) AddTaskImpl(std::shared_ptr<std::packaged_task<R()>> task);

private:
	std::queue<Delegate<void()>> tasks;
	std::vector<std::thread> threads;
	std::thread::id mainThreadId;
	std::condition_variable cv;
	std::mutex taskMutex;
	bool isEnd = false;
};

template <class R, class... Args>
decltype(auto) ThreadManager::AddTask(const Delegate<R(Args...)>& task, Args&&... args) noexcept
{
	auto package = std::make_shared<std::packaged_task
		<R()>>(std::bind(task, std::forward<Args>(args)...));

	return AddTaskImpl(std::move(package));
}

template <class R, class... Args>
decltype(auto) ThreadManager::AddTask(Delegate<R(Args...)>&& task, Args&&... args) noexcept
{
	auto package = std::make_shared<std::packaged_task
		<R()>>(std::bind(std::move(task), std::forward<Args>(args)...));

	return AddTaskImpl(std::move(package));
}

template <class R>
decltype(auto) ThreadManager::AddTaskImpl(std::shared_ptr<std::packaged_task<R()>> task)
{
	taskMutex.lock();
	tasks.push([task] { (*task)(); });
	taskMutex.unlock();

	cv.notify_one();
	return task->get_future();
}
