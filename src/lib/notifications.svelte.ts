export interface Notification {
  id: string;
  type: 'error' | 'success' | 'info' | 'warning';
  message: string;
}

let list = $state<Notification[]>([]);

export const notifications = {
  get list() {
    return list;
  },
  show(message: string, type: Notification['type'] = 'info', duration = 5000) {
    if (!message) return;
    const id = crypto.randomUUID();
    list.push({ id, type, message });
    if (duration > 0) {
      setTimeout(() => {
        this.dismiss(id);
      }, duration);
    }
  },
  error(message: string, duration = 8000) {
    this.show(message, 'error', duration);
  },
  success(message: string, duration = 4000) {
    this.show(message, 'success', duration);
  },
  info(message: string, duration = 4000) {
    this.show(message, 'info', duration);
  },
  warning(message: string, duration = 5000) {
    this.show(message, 'warning', duration);
  },
  dismiss(id: string) {
    list = list.filter(n => n.id !== id);
  }
};
