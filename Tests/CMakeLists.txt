# Tests

cmake_minimum_required (VERSION 3.12)

project (BSEngine-Tests
	VERSION 0.1.0
	LANGUAGES CXX
)

if (GTest_FOUND)
	file(GLOB_RECURSE TEST_FILES "*.cpp")
	add_executable(BSEngine-Tests ${TEST_FILES})
	target_link_libraries(BSEngine-Tests PRIVATE Core GTest::GTest)

	enable_testing()
	add_test(NAME BSEngine-Test COMMAND BSEngine-Tests)
endif ()