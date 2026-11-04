package com.patrickhaahr.openhome.ui

import androidx.annotation.DrawableRes
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.dropUnlessResumed
import androidx.navigation.NavDestination
import androidx.navigation.NavDestination.Companion.hasRoute
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.navigation
import androidx.navigation.compose.rememberNavController
import androidx.navigation.toRoute
import com.patrickhaahr.openhome.R
import com.patrickhaahr.openhome.ui.home.HomeScreen
import com.patrickhaahr.openhome.ui.profile.ProfileRoute
import com.patrickhaahr.openhome.ui.workout.HistoryRoute
import com.patrickhaahr.openhome.ui.workout.WorkoutDetailRoute
import com.patrickhaahr.openhome.ui.workout.WorkoutRoute
import kotlinx.serialization.Serializable

private const val TRANSITION_MILLIS = 150

@Serializable
private data object WorkoutTab

@Serializable
private data object WorkoutHome

@Serializable
private data object History

@Serializable
private data class WorkoutDetail(val id: Long)

@Serializable
private data object HomeTab

@Serializable
private data object HomeRoot

@Serializable
private data object Profile

private enum class Tab(val graph: Any, val root: Any, val label: String, @param:DrawableRes val icon: Int) {
    Workout(WorkoutTab, WorkoutHome, "Workout", R.drawable.ic_workout),
    Home(HomeTab, HomeRoot, "Home", R.drawable.ic_home),
}

private fun NavDestination?.isIn(tab: Tab): Boolean =
    this?.hierarchy?.any { destination ->
        when (tab) {
            Tab.Workout -> destination.hasRoute<WorkoutTab>()
            Tab.Home -> destination.hasRoute<HomeTab>()
        }
    } == true

/**
 * The app's navigation hierarchy on one back stack, so system back and the back gesture pop nested
 * views. Workout is the start destination: back at a tab root returns there, and back on Workout's
 * root leaves the activity to Android's default behavior. Destinations navigate onward only while
 * resumed, so a second tap during a transition cannot stack a duplicate.
 */
@Composable
fun MainNavigation() {
    val navController = rememberNavController()
    val backStackEntry by navController.currentBackStackEntryAsState()
    val destination = backStackEntry?.destination
    val inTab = Tab.entries.any { destination.isIn(it) }

    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        bottomBar = {
            if (inTab) {
                BottomTabs(destination, onSelect = { tab -> navController.openTab(tab, destination) })
            }
        },
    ) { padding ->
        NavHost(
            navController = navController,
            startDestination = WorkoutTab,
            modifier = Modifier.padding(padding),
            enterTransition = { fadeIn(tween(TRANSITION_MILLIS)) },
            exitTransition = { fadeOut(tween(TRANSITION_MILLIS)) },
        ) {
            navigation<WorkoutTab>(startDestination = WorkoutHome) {
                composable<WorkoutHome> {
                    WorkoutRoute(
                        onOpenHistory = dropUnlessResumed { navController.navigate(History) },
                        onOpenProfile = dropUnlessResumed { navController.navigate(Profile) },
                    )
                }
                composable<History> { entry ->
                    HistoryRoute(
                        onOpenWorkout = { id ->
                            if (entry.lifecycle.currentState.isAtLeast(Lifecycle.State.RESUMED)) {
                                navController.navigate(WorkoutDetail(id))
                            }
                        },
                    )
                }
                composable<WorkoutDetail> { entry ->
                    WorkoutDetailRoute(workoutId = entry.toRoute<WorkoutDetail>().id)
                }
            }
            navigation<HomeTab>(startDestination = HomeRoot) {
                composable<HomeRoot> {
                    HomeScreen(onOpenProfile = dropUnlessResumed { navController.navigate(Profile) })
                }
            }
            composable<Profile> { ProfileRoute() }
        }
    }
}

@Composable
private fun BottomTabs(destination: NavDestination?, onSelect: (Tab) -> Unit) {
    NavigationBar(containerColor = MaterialTheme.colorScheme.surfaceContainerLow) {
        Tab.entries.forEach { tab ->
            NavigationBarItem(
                selected = destination.isIn(tab),
                onClick = { onSelect(tab) },
                icon = { Icon(painterResource(tab.icon), contentDescription = null) },
                label = { Text(tab.label) },
                colors =
                    NavigationBarItemDefaults.colors(
                        selectedIconColor = MaterialTheme.colorScheme.onPrimary,
                        indicatorColor = MaterialTheme.colorScheme.primary,
                        selectedTextColor = MaterialTheme.colorScheme.onSurface,
                        unselectedIconColor = MaterialTheme.colorScheme.onSurfaceVariant,
                        unselectedTextColor = MaterialTheme.colorScheme.onSurfaceVariant,
                    ),
            )
        }
    }
}

/** Keeps each tab's nested state; choosing the tab already open returns to its root. */
private fun NavHostController.openTab(tab: Tab, current: NavDestination?) {
    if (current.isIn(tab)) {
        popBackStack(tab.root, inclusive = false)
        return
    }
    navigate(tab.graph) {
        popUpTo(graph.findStartDestination().id) { saveState = true }
        launchSingleTop = true
        restoreState = true
    }
}
