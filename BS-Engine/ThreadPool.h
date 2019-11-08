#pragma once

#include "Macro.h"
#include "Array.h"
#include "Queue.h"
#include <thread>
#include <functional>
#include <mutex>

class BS_API ThreadPool final
{
public:
	ThreadPool() noexcept;
	~ThreadPool();

private:
	void Work() noexcept;

private:
	Array<std::thread> threads;
	Queue<std::function<void()>> job;
	std::condition_variable cv;
	std::mutex jobMutex;
	bool isEnd;
};