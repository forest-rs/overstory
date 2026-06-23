#import "AppDelegate.h"

#import "OverstoryDemoUIKitFFI.h"

@implementation AppDelegate

- (BOOL)application:(UIApplication *)application
    didFinishLaunchingWithOptions:(NSDictionary *)launchOptions {
  (void)application;
  (void)launchOptions;

  UIViewController *rootViewController =
      overstory_demo_uikit_create_root_view_controller();
  if (rootViewController == nil) {
    return NO;
  }

  self.window = [[UIWindow alloc] initWithFrame:UIScreen.mainScreen.bounds];
  self.window.rootViewController = rootViewController;
  [self.window makeKeyAndVisible];
  return YES;
}

@end
