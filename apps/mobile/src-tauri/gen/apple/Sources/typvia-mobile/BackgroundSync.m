// Background refresh registration. BGTaskScheduler demands that all launch
// handlers exist before the app finishes launching; tao owns the
// application delegate and offers no extension point there, so registration
// happens in +load — guaranteed before main() runs.

#import <BackgroundTasks/BackgroundTasks.h>
#import <UIKit/UIKit.h>

// Rust entry (apps/mobile/src-tauri/src/background.rs). Returns false while
// the app has not finished assembling (cold background launch racing setup).
extern bool typvia_background_round(void);

static NSString *const TypviaRefreshTaskId = @"dev.typvia.mobile.refresh";

// Earliest-begin interval for the re-submitted request; the actual cadence
// stays the system's call (best-effort semantics).
static NSTimeInterval const TypviaRefreshInterval = 30 * 60;

@interface TypviaBackgroundSync : NSObject
@end

@implementation TypviaBackgroundSync

+ (void)load {
  [[BGTaskScheduler sharedScheduler]
      registerForTaskWithIdentifier:TypviaRefreshTaskId
                         usingQueue:nil
                      launchHandler:^(BGTask *task) {
                        [TypviaBackgroundSync
                            handleRefresh:(BGAppRefreshTask *)task];
                      }];
  // First submission happens when the app leaves the foreground — the
  // standard arming point; every handler run re-arms itself below.
  [[NSNotificationCenter defaultCenter]
      addObserverForName:UIApplicationDidEnterBackgroundNotification
                  object:nil
                   queue:nil
              usingBlock:^(NSNotification *note) {
                [TypviaBackgroundSync scheduleNext];
              }];
}

+ (void)handleRefresh:(BGAppRefreshTask *)task {
  // Re-arm before running: a round interrupted by expiration must not also
  // lose its next slot.
  [TypviaBackgroundSync scheduleNext];
  __block BOOL expired = NO;
  task.expirationHandler = ^{
    // The round is idempotent and safe to abandon mid-flight (watermark and
    // transaction discipline); just report the truncation honestly.
    expired = YES;
  };
  dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
    bool ran = typvia_background_round();
    [task setTaskCompletedWithSuccess:(ran && !expired)];
  });
}

+ (void)scheduleNext {
  BGAppRefreshTaskRequest *request =
      [[BGAppRefreshTaskRequest alloc] initWithIdentifier:TypviaRefreshTaskId];
  request.earliestBeginDate =
      [NSDate dateWithTimeIntervalSinceNow:TypviaRefreshInterval];
  // Silent on failure by design: with Background App Refresh switched off
  // the request is simply not schedulable, and the foreground triggers
  // still cover every sync path.
  [[BGTaskScheduler sharedScheduler] submitTaskRequest:request error:nil];
}

@end
