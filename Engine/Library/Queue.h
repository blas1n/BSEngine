#pragma once

#include "Core.h"
#include <queue>

namespace BE
{
	template <class T, class InAllocator>
	class BS_API Queue final
	{
	public:
		using ElementType = T;

		Queue() noexcept : container{ } { }

		Queue(const Queue& other) : container{ other.container } {}
		Queue(Queue&& other) noexcept : container{ std::move(other.container) } {}

		~Queue() = default;

		inline Queue& operator=(const Queue& other)
		{
			container = other.container;
			return *this;
		}

		inline Queue& operator=(Queue&& other) noexcept
		{
			container = std::move(other.container);
			return *this;
		}

		inline T& Front()
		{
			return container.front();
		}

		inline const T& Front() const
		{
			return container.front();
		}

		inline T& Back()
		{
			return container.back();
		}

		inline const T& Back() const
		{
			return container.back();
		}

		inline bool IsEmpty() const noexcept
		{
			return container.empty();
		}

		inline SizeType GetSize() const noexcept
		{
			return container.size();
		}

		inline void Enqueue(const T& value)
		{
			container.push(value);
		}

		inline void Enqueue(T&& value)
		{
			container.push(std::move(value));
		}

		template <class... Args>
		inline void Emplace(Args&&... args)
		{
			container.emplace(std::forward<Args>(args)...);
		}

		inline void Dequeue()
		{
			container.pop();
		}

	private:
		std::queue<T> container;
	};
}