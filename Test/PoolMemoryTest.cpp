#include "pch.h"
#include "PoolMemory.h"

TEST(PoolMemory, Create)
{
	PoolMemory<1> pool{ 10 };
	ASSERT_EQ(pool.GetMaxByte(), 10);
}

TEST(PoolMemory, Malloc)
{
	PoolMemory<sizeof(int)> pool{ 5 };
	auto p = pool.Malloc(6);
	ASSERT_EQ(p, nullptr);

	p = pool.Malloc(3);
	ASSERT_TRUE(p != nullptr);

	p = pool.Malloc(3);
	ASSERT_EQ(p, nullptr);

	p = pool.Malloc(2);
	ASSERT_EQ(pool.GetAssignableByte(), 0);
}

TEST(PoolMemory, Free)
{
	PoolMemory<sizeof(int)> pool{ 5 };
	ASSERT_DEATH(pool.Free(nullptr), "Assertion failed");

	for (auto i = 5; i > 0; --i)
	{
		auto p = pool.Malloc(i);
		pool.Free(p);
		ASSERT_EQ(pool.GetAssignedByte(), 0);
	}

	const auto p = static_cast<unsigned char*>(pool.Malloc(1)) - 1;
	ASSERT_DEATH(pool.Free(p), "Assertion failed");
}