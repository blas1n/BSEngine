#pragma once

#include "MemoryManager.h"
#include "Macro.h"
#include "Type.h"
#include <type_traits>

namespace BE
{
	class BS_API AllocatorImpl : public MemoryManagerAccesser
	{
	protected:
		void* Alloc(const size_t n) noexcept;
		void Dealloc(void* ptr, const size_t n) noexcept;
	};

	/**
	 * @brief Templated custom allocator.
	 * @detail I cannot add 'final' keyword.
	 * See https://stackoverflow.com/questions/55310209/can-a-c-allocator-be-final
	 * @todo Add 'final' keyword after make custom container.
	*/
	template <class T>
	class BS_API Allocator : private AllocatorImpl
	{
	public:
		using value_type = T;
		using size_type = size_t;
		using difference_type = std::ptrdiff_t;
		using propagate_on_container_move_assignment = std::true_type;
		using is_always_equal = std::true_type;

		inline Allocator() noexcept = default;
		inline Allocator(const Allocator& other) noexcept = default;
		inline ~Allocator() noexcept = default;

		template <class U>
		inline Allocator(const Allocator<U>& other) noexcept {}

		/**
		 * @brief Allocate memory.
		 * @param n Number of objects to be allocated.
		 * @return Allocated object pointer.
		 * @retval nullptr Can not allocate.
		 * @remark It just allocates memory but does not call the constructor.
		*/
		inline T* allocate(const size_t n = 1) noexcept
		{
			return static_cast<T*>(Alloc(n * sizeof(T)));
		}

		/**
		 * @brief Deallocate memory.
		 * @param ptr Pointer to be deallocated.
		 * @param n Number of objects to be deallocated.
		 * @return Allocated object pointer.
		 * @retval nullptr Can not allocate.
		 * @remark It just allocates memory but does not call the constructor.
		*/
		inline void deallocate(T* ptr, const size_t n = 1) noexcept
		{
			Dealloc(static_cast<void*>(ptr), n * sizeof(T));
		}
	};
}